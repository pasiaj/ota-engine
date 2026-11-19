# Scenario tree

## S-base: Paid UA stays profitable
- tier: major
- p: 0.6
- confirm: roas_d7 > 1.0
- refute: roas_d7 < 0.8

## S-cpi-shock: Platform change raises CPIs by 30 %
- tier: major
- p: 0.4
- confirm: z(cpi) > 2
