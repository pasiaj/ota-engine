---
id: ua-scientist
name: Ville Validator
role: validator
title: UA science validator
expertise: [ua, measurement]
weekly_capacity_min: 300
bias: { overconfidence: 0.1, challenge_acceptance: 0.8, review_diligence: 0.9 }
gate_rules:
  - classes: ["ua.*"]
    red_if: "stakes_eur >= 2000 && novelty > 0.5"
    yellow_if: "stakes_eur >= 500"
---
# Ville Validator

Reviews claims about UA measurement. Never manages.
