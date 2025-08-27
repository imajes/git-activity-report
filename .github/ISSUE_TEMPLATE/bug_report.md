name: Bug report
description: Report a problem with git-activity-report
labels: [bug]
body:
  - type: textarea
    attributes:
      label: Summary
      description: What happened? What did you expect?
    validations:
      required: true
  - type: textarea
    attributes:
      label: Reproduction steps
      description: Include exact commands and sample repos/fixtures if possible
      value: |
        1.
        2.
        3.
  - type: textarea
    attributes:
      label: Environment
      description: OS, Python/Rust versions, Git version
  - type: textarea
    attributes:
      label: Logs/output
      description: Relevant stderr/stdout, validation errors, or stack traces
  - type: checkboxes
    attributes:
      label: Checks
      options:
        - label: I ran `just validate-all` and included errors
        - label: I ran `just test` and included diffs
