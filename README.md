# pd-assignee
Choose GitHub assignee from PagerDuty schedule.

## Usage
pd-assignee is intended to be used in GitHub Actions.

```yaml
on:
  issues:
    types: [labeled]

jobs:
  choose-assignee:
    runs-on: ubuntu-latest
    # Choose assignee if the issue has "bug" label and no assignees.
    if: ${{ github.event.label.name == 'bug' && toJSON(github.event.issue.assignees) == '[]' }}
    permissions:
      # Need issues.write to add assignee
      issues: write
    env:
      PD_ASSIGNEE_VERSION: v0.1.2
      PD_ASSIGNEE_TARGET: x86_64-unknown-linux-gnu
    steps:
      - id: cache-pd-assignee
        uses: actions/cache@v3
        with:
          path: ${{ runner.temp }}/pd-assignee
          key: ${{ env.PD_ASSIGNEE_VERSION }}-${{ env.PD_ASSIGNEE_TARGET }}
      - if: steps.cache-pd-assignee.outputs.cache-hit != 'true'
        env:
          BINDIR: '${{ runner.temp }}/pd-assignee'
        run: |
          mkdir -p "$BINDIR"
          pushd "$BINDIR"
          curl -sSfLO "https://github.com/eagletmt/pd-assignee/releases/download/${PD_ASSIGNEE_VERSION}/pd-assignee-${PD_ASSIGNEE_VERSION}-${PD_ASSIGNEE_TARGET}"
          curl -sSfL "https://github.com/eagletmt/pd-assignee/releases/download/${PD_ASSIGNEE_VERSION}/SHA256SUMS.txt" | sha256sum --check --ignore-missing
          mv "pd-assignee-${PD_ASSIGNEE_VERSION}-${PD_ASSIGNEE_TARGET}" pd-assignee
          chmod +x pd-assignee
          popd
      - run: echo '${{ runner.temp }}/pd-assignee' >> $GITHUB_PATH
      - id: pd-assignee
        env:
          PAGERDUTY_API_KEY: ${{ secrets.PAGERDUTY_API_KEY }}
          # Use --pagerduty-schedule-id option to use multiple schedules
          PAGERDUTY_SCHEDULE_ID: P012345
          # Set GITHUB_ENDPOINT if you're using GitHub Enterprise Server
          # GITHUB_ENDPOINT: https://ghes.example.com/api/graphql
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          GITHUB_ORG: my-org
          GITHUB_TEAM_SLUG: my-team
          DEFAULT_ASSIGNEE: eagletmt
        run: pd-assignee >> $GITHUB_OUTPUT
      - env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        run: gh issue edit ${{ github.event.issue.number }} --add-assignee ${{ steps.pd-assignee.outputs.assignee }} --repo ${{ github.repository }}
```
