## Decision class: ua.budget_shift
- reversibility: costly_to_reverse
- impact: cross_function
- accountable: ceo
- validator: ua-scientist
- kind: scale
- stakes_eur: 2500
- novelty: 0.6
- assumptions: [A-roas, A-retention]
- hypothesis: "Doubling budget keeps ROAS above 1.0"
- learning_metric: roas_d7
- decision_point: "Day 14 ROAS readout"
- commitment_point: "Budget committed for two weeks"
- confirm_if: "roas_d7 >= 1.0"
- stop_if: "roas_d7 < 0.8"
- check_after_days: 14

| when                                                | then                         | confidence | escalate |
|-----------------------------------------------------|------------------------------|-----------:|----------|
| `roas_d7 > 1.3 && days_since(ua.budget_shift) > 21` | set_budget(daily=spend * 1.5) | 0.60       | large    |
| otherwise                                           | hold                         | 0.70       |          |
