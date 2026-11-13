# Merge a single UTC commit through a PR

Use a feature branch, PR, successful Source checks, then GitHub **Rebase and merge** with an explicit UTC request header. Do not use provider squash for this route. Do not push main directly or rewrite existing history.

1. Prepare exactly one feature commit before the branch's first push, using the approved public identity and explicit `GIT_AUTHOR_DATE` and `GIT_COMMITTER_DATE` values ending in `+0000`
2. Push the feature branch, open its PR and wait for Source checks
3. Fetch the exact checked source commit into the existing checkout
4. After merge authorization, run `node scripts/merge-pr-utc.mjs PR_NUMBER CHECKED_HEAD_SHA`
5. Verify the returned raw commit proof before using its SHA for CI or package provenance

The helper requires exactly one commit in the PR and local range, both source timestamp offsets in UTC, a source parent equal to the checked main base, successful Source checks on the pinned head, and normal mergeability. It submits the ordinary merge endpoint with `merge_method=rebase`, the exact head SHA and `Time-Zone: Etc/UTC`. It never updates a Git ref through the API or pushes main.

After the provider confirms a merge, the helper fetches the returned commit and verifies both raw author and committer offsets, the unchanged source tree and the same sole parent. This proves one intended commit was added without extra changes. Rebase and merge can change the commit SHA. GitHub REST dates ending in `Z` do not prove the stored offsets.

An API error leaves the merge outcome unconfirmed: inspect the PR before any retry. A post-merge proof failure means the merge has already happened: stop and report without retrying or rewriting history. These checks detect unexpected outcomes, not prevent a provider write that already happened.

The earlier squash route did not preserve both UTC offsets. This rebase route requires its first real validation on this process PR; local mocks do not prove provider behavior. Preserve that live evidence separately rather than merging another change merely to record success. If either raw offset is not UTC, stop instead of trying another merge strategy without authorization.

If review requires changes after the first push, do not amend or rebase pushed commits to meet this helper's one-commit rule. Stop and arrange an approved new single-commit branch flow. Keep existing release tags and accepted packages unchanged by process-only updates.

References:

- [GitHub timezone request rules](https://docs.github.com/en/rest/using-the-rest-api/timezones-and-the-rest-api)
- [GitHub PR merge endpoint](https://docs.github.com/en/rest/pulls/pulls#merge-a-pull-request)
- [GitHub merge strategies](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/incorporating-changes-from-a-pull-request/about-pull-request-merges)
