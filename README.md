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
    # Choose assignee if the issue has "bug" label and no assignees.
    if: ${{ github.event.label.name == 'bug' && toJSON(github.event.issue.assignees) == '[]' }}
    env:
      PAGERDUTY_API_KEY: ${{ secrets.PAGERDUTY_API_KEY }}
      # Use --pagerduty-schedule-id option to use multiple schedules
      PAGERDUTY_SCHEDULE_ID: P012345
      # Set GITHUB_ENDPOINT if you're using GitHub Enterprise Server
      # GITHUB_ENDPOINT: https://ghes.example.com/api/graphql
      GITHUB_ORG: my-org
      GITHUB_TEAM_SLUG: my-team
      DEFAULT_ASSIGNEE: eagletmt@gmail.com
    steps:
      - id: pd-assignee
        run: pd-assignee >> $GITHUB_OUTPUT
      - run: gh issue edit ${{ github.event.issue.number }} --add-assignee ${{ steps.pd-assignee.outputs.assignee }}
```
