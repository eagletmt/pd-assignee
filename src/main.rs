use clap::Parser as _;
use graphql_client::GraphQLQuery as _;

#[derive(Debug, clap::Parser)]
struct Args {
    #[arg(long, env, default_value = "https://api.pagerduty.com")]
    pagerduty_endpoint: reqwest::Url,
    #[arg(long, env, hide_env_values = true)]
    pagerduty_api_key: String,
    #[arg(short = 's', long, env)]
    pagerduty_schedule_id: Vec<String>,

    #[arg(long, env, hide_env_values = true)]
    github_token: String,
    #[arg(long, env, default_value = "https://api.github.com/graphql")]
    github_endpoint: String,
    #[arg(short = 'o', long, env)]
    github_org: String,
    #[arg(short = 't', long, env)]
    github_team_slug: String,

    #[arg(long)]
    at: Option<chrono::DateTime<chrono::Utc>>,
    #[arg(long, env)]
    default_assignee: Option<String>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let args = Args::parse();

    let github_client = build_github_client(&args.github_token)?;
    let mut team_members = list_github_members(
        &github_client,
        &args.github_endpoint,
        args.github_org,
        args.github_team_slug,
    )
    .await?;
    tracing::debug!(?team_members, "Fetched team members");

    let pagerduty_client = build_pagerduty_client(&args.pagerduty_api_key)?;

    let at = args.at.unwrap_or_else(chrono::Utc::now).to_rfc3339();
    let mut assignee = None;
    for schedule_id in args.pagerduty_schedule_id {
        tracing::debug!(%schedule_id, "Get oncall schedule");
        let user_id = get_oncall(
            &pagerduty_client,
            &args.pagerduty_endpoint,
            &schedule_id,
            &at,
        )
        .await?;
        let Some(user_id) = user_id else {
            tracing::warn!(%schedule_id, "Cannot find final schedule entry");
            continue;
        };
        tracing::debug!(%user_id, "Oncall user found");

        let email = get_user(&pagerduty_client, &args.pagerduty_endpoint, &user_id).await?;

        if let Some(login) = team_members.remove(&email) {
            assignee = Some(login);
            break;
        } else {
            tracing::debug!(%email, "Oncall user doesn't belong to the team");
        }
    }

    let Some(assignee) = assignee.or_else(|| {
        args.default_assignee
            .and_then(|email| team_members.remove(&email))
    }) else {
        anyhow::bail!("Cannot find assignee");
    };
    println!("assignee={assignee}");
    Ok(())
}

fn build_github_client(token: &str) -> anyhow::Result<reqwest::Client> {
    let mut default_headers = reqwest::header::HeaderMap::new();
    default_headers.insert(
        reqwest::header::AUTHORIZATION,
        format!("bearer {token}").parse()?,
    );
    Ok(reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(20))
        .default_headers(default_headers)
        .user_agent(concat!(
            env!("CARGO_PKG_NAME"),
            "/",
            env!("CARGO_PKG_VERSION")
        ))
        .build()?)
}

#[derive(Debug, graphql_client::GraphQLQuery)]
#[graphql(
    query_path = "list_members_query.graphql",
    schema_path = "schema.docs.graphql"
)]
struct ListMembersQuery;

async fn list_github_members(
    client: &reqwest::Client,
    endpoint: &str,
    owner: String,
    team_slug: String,
) -> anyhow::Result<std::collections::HashMap<String, String>> {
    let resp: graphql_client::Response<list_members_query::ResponseData> = client
        .post(endpoint)
        .json(&ListMembersQuery::build_query(
            list_members_query::Variables { owner, team_slug },
        ))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    if let Some(errors) = resp.errors {
        anyhow::bail!("ListMembersQuery failed: {:?}", errors);
    }
    let Some(nodes) = resp.data
        .and_then(|data| data.organization)
        .and_then(|org| org.team)
        .and_then(|team| team.members.nodes)
    else {
        anyhow::bail!("ListMembersQuery returned empty data");
    };
    Ok(std::collections::HashMap::from_iter(
        nodes
            .into_iter()
            .flatten()
            .map(|node| (node.email, node.login)),
    ))
}

fn build_pagerduty_client(api_key: &str) -> anyhow::Result<reqwest::Client> {
    let mut default_headers = reqwest::header::HeaderMap::new();
    default_headers.insert(
        reqwest::header::AUTHORIZATION,
        format!("Token token={api_key}").parse()?,
    );
    default_headers.insert(
        reqwest::header::ACCEPT,
        reqwest::header::HeaderValue::from_static("application/vnd.pagerduty+json;version=2"),
    );
    Ok(reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(20))
        .default_headers(default_headers)
        .user_agent(concat!(
            env!("CARGO_PKG_NAME"),
            "/",
            env!("CARGO_PKG_VERSION")
        ))
        .build()?)
}

#[derive(Debug, serde::Deserialize)]
struct GetScheduleResponse {
    schedule: Schedule,
}
#[derive(Debug, serde::Deserialize)]
struct Schedule {
    final_schedule: SubSchedule,
}
#[derive(Debug, serde::Deserialize)]
struct SubSchedule {
    rendered_schedule_entries: Vec<ScheduleLayerEntry>,
}
#[derive(Debug, serde::Deserialize)]
struct ScheduleLayerEntry {
    user: UserReference,
}
#[derive(Debug, serde::Deserialize)]
struct UserReference {
    id: String,
}

async fn get_oncall(
    client: &reqwest::Client,
    endpoint: &reqwest::Url,
    schedule_id: &str,
    at: &str,
) -> anyhow::Result<Option<String>> {
    let mut schedule_url = endpoint.join(&format!("schedules/{schedule_id}")).unwrap();
    schedule_url
        .query_pairs_mut()
        .append_pair("since", at)
        .append_pair("until", at);
    let resp: GetScheduleResponse = client
        .get(schedule_url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(resp
        .schedule
        .final_schedule
        .rendered_schedule_entries
        .into_iter()
        .next()
        .map(|entry| entry.user.id))
}

#[derive(Debug, serde::Deserialize)]
struct GetUserResponse {
    user: User,
}
#[derive(Debug, serde::Deserialize)]
struct User {
    email: String,
}

async fn get_user(
    client: &reqwest::Client,
    endpoint: &reqwest::Url,
    user_id: &str,
) -> anyhow::Result<String> {
    let resp: GetUserResponse = client
        .get(endpoint.join(&format!("users/{user_id}")).unwrap())
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(resp.user.email)
}
