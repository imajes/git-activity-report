# Feedback after running in a real repository

## Commit Object Notes

1. `patch` should become a `Vec<String>` (split on new lines) as opposed to a single string.
2. `timezone` should state the actual timezone (so, `local` here is `America/Chicago` or `CDT`).
3. Also, we need to extend `-tz` to allow any timezone to be given, allowing a user to push time into their preferred tz.
4. For `patch_ref`, first, lets rename to `patch_references`. then, i'd like to actually see the github urls completed.
   a. I want to see a new structure `patch_references.github`, and under that the following urls.
   b. these should be easy to generate by looking at the git remote, and constructing based on standard uris:
   - `commit_url`: <https://github.com/imajes/git-activity-report/commit/5bc0f7e59a97e23b5c646ca22df318cf0983702d>
   - `diff_url`: <https://github.com/imajes/git-activity-report/commit/5bc0f7e59a97e23b5c646ca22df318cf0983702d.diff>
   - `patch_url`: <https://github.com/imajes/git-activity-report/commit/5bc0f7e59a97e23b5c646ca22df318cf0983702d.patch>

### Within commit, the `github_prs` Key

1. this key should now change. It should become `github.pull_requests` - i considered it under `enrichment`, but i'm not convinced. I'd be curious of thoughts.
2. the `body` key under the current `github_prs` should become `Vec<String>` split on new lines.
3. it needs an `approver` object, which lists the github approver user, and as far as i can tell, the `user` object doesn't make sense now?
4. if possible, it needs also an array of `reviewers`.
5. For the `submitter`, `reviewers` and `approver` objects, they should continue to be formed by a `GithubUser`, but it should
   a. Include a new property called `profile_url`, which would display their `https://github.com/<username>` link;
   b. Include a `type` prop, enum of `bot`, `contributor`, `ai-agent`, etc.
   c. We should also include the user's email, if possible.

## Overall `summary` Keys

1. in the summary context, i want to see the following nested under `summary`, and the existing `summary` key renamed to `changeset`:

   ```json
     "count": 49,
     "include_merges": false,
     "include_patch": false,
     "range": {
       "since": "the last month",
       "until": "now"
     },
     "repo": "/Users/james/Projects/clients/tip411/rails/webapp",
     "summary": {
       "additions": 2083,
       "deletions": 1505,
       "files_touched": 144
     }
   ```

2. the `range` property should be changed. It needs to reflect the range window objects we have:
   a. Range should be an object that has the `label`, `start` (replace `since`) and `end` (replaces `until`)
   b. `label` reflects the language used initially, so e.g. "last month".
   c. `start` and `end` should be a correctly formed timestamp ALWAYS - derived from the range calculation.

3. When there are multiple ranges (the bucket play), in the manifest, `range` should become `ranges`, containing multiple ranges. I need to run a few of these to validate the structure, so don't go too much further here.
4. The `include_..` keys should be nested under `report_options`, and _all_ options, including the way it was called, should be present. (i.e. it's the caller invocation plus the option set that was parsed from it).

## Issues running `--for`

1. I ran `git activity-report --tz local --out reports/for-buckets --for "each month for the last six"` which generated literally that file (`reports/for-buckets`) and had the following content:

   ```json
   {
     "authors": {},
     "commits": [],
     "count": 0,
     "include_merges": false,
     "include_patch": false,
     "range": {
       "since": "each month for the last six",
       "until": "now"
     },
     "repo": "/Users/james/Projects/clients/tip411/rails/webapp",
     "summary": {
       "additions": 0,
       "deletions": 0,
       "files_touched": 0
     }
   }
   ```

2. So, when a `--for` statement ends up with zero commits, it should error out and return this information to stdout rather than the file.
3. also, shouldn't that `--for` clause have worked?
4. these needs to be fixed; i thought we had a mkdir for this?

   ```shell
    ❯ git activity-report --tz local --out reports/forbucket/ --for "the last month"
    Error: No such file or directory (os error 2)

    ❯ git activity-report --tz local --out reports/ --for "the last month"
    Error: Is a directory (os error 21)
   ```

## other comments

1. i think it's probably time to output to `stdout` [UNLESS that's the intended destination for the output, then suppress or use stderr?] information about what it's doing at any given time. this is because it takes quite a long time to run, and it's nicer to have some sense of progress. A throbber/spinner would be nice too.
