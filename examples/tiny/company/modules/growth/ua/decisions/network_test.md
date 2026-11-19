## Decision class: ua.network_test
- reversibility: reversible
- impact: local
- kind: test
- stakes_eur: 400
- novelty: 0.5
- hypothesis: "A second network delivers CPI within 20 % of the main one"
- learning_metric: cpi
- decision_point: "after 4 days of spend"
- check_after_days: 4
- confirm_if: "cpi < 2.4"
- stop_if: "cpi > 3.5"

| when                                                       | then                        | confidence | escalate |
|------------------------------------------------------------|-----------------------------|-----------:|----------|
| `days_since(ua.network_test) > 10 && trend(cpi, 7) > 0.01` | start_test(network="nova")  | 0.50       |          |
| otherwise                                                  | hold                        | 0.70       |          |
