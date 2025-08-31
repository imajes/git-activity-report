name: Feature request
description: Suggest an idea for git-activity-report
labels: [enhancement]
body:

- type: textarea
    attributes:
      label: Problem
      description: What problem does this feature solve?
- type: textarea
    attributes:
      label: Proposal
      description: Describe the solution and any alternatives considered
- type: textarea
    attributes:
      label: Output/Schema impact
      description: Which schemas might change? Include examples
- type: checkboxes
    attributes:
      label: Backwards compatibility
      options:
        - label: Change is additive (preferred)
        - label: Requires a new schema version
