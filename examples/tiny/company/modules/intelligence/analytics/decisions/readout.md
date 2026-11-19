## Decision class: analytics.readout
- reversibility: reversible
- impact: local
- kind: routine
- stakes_eur: 0
- assumptions: [A-retention]
- forecast: "DAU stays within 2 sigma tomorrow"
- forecast_if: "abs(z(dau)) < 2"
- forecast_horizon_days: 1
- check_after_days: 1
- confirm_if: "abs(z(dau)) < 2"
- stop_if: "abs(z(dau)) > 4"

| when              | then                          | confidence | escalate |
|-------------------|-------------------------------|-----------:|----------|
| `abs(z(dau)) > 3` | readout(severity=3)           | 0.55       | small    |
| `abs(z(dau)) > 2` | readout(severity=2)           | 0.65       |          |
| otherwise         | hold                          | 0.80       |          |
