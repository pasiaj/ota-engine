# Bid adjustments

## Decision class: ua.bid_adjust
- reversibility: reversible
- impact: local
- owner: ua            # module
- accountable: ceo     # named human
- default_rung: act_then_review
- stakes_eur: 300
- novelty: 0.2
- assumptions: [A-roas]
- forecast: "D7 ROAS of the adjusted bid ≥ 1.0 in 7 days"
- forecast_if: "roas_d7 >= 1.0"
- forecast_horizon_days: 7
- confirm_if: "roas_d7 >= 1.0"
- stop_if: "roas_d7 < 0.7"
- check_after_days: 7

| when                                     | then          | confidence | escalate |
|------------------------------------------|---------------|-----------:|----------|
| `roas_d7 > 1.15 && spend_util > 0.9`     | bid *= 1.10   | 0.72       |          |
| `roas_d7 < 0.80`                         | bid *= 0.85   | 0.70       |          |
| `z(cpi) > 3`                             | hold          | 0.40       | small    |
| otherwise                                | hold          | 0.60       |          |
