# Merge PRs with UTC commit metadata

Keep the feature-branch, PR, Source checks and squash workflow. Do not push main directly or rewrite merged history.

1. Create feature commits with the approved public identity and explicit `GIT_AUTHOR_DATE` and `GIT_COMMITTER_DATE` values ending in `+0000`
2. Push the feature branch, open its PR and wait for Source checks
3. Fetch the checked feature commits into the existing checkout
4. Run `node scripts/merge-pr-utc.mjs PR_NUMBER CHECKED_HEAD_SHA` after merge authorization
5. Use the returned merge SHA for later CI and package provenance

The script refuses changed heads, draft or non-main PRs, non-UTC feature metadata, and missing or unsuccessful Source checks. It uses the ordinary GitHub squash endpoint with a pinned head and the explicit `Time-Zone: Etc/UTC` header. It never updates a Git ref through the API or pushes main.

The request header sets the intended timestamp timezone. The raw post-merge check verifies both author and committer offsets. GitHub REST dates ending in `Z` alone do not prove the stored offsets. If post-merge verification fails, the merge has already happened. Stop and report it without retrying or rewriting history. Do not describe that detection as prevention or claim successful UTC verification.

This process change needs its first live validation on its own normal PR. Do not assume every endpoint honors the timezone header merely because the general API documentation describes it. Preserve the release tag and accepted package independently of later process-only commits.

References:

- [GitHub timezone request rules](https://docs.github.com/en/rest/using-the-rest-api/timezones-and-the-rest-api)
- [GitHub PR merge endpoint](https://docs.github.com/en/rest/pulls/pulls#merge-a-pull-request)
