# Strategy claims

## C-mission: Make small games people return to every day
- kind: mission

## C-growth: Paid UA is a growth lever, organic is the base
- kind: strategy
- links: [C-mission]

## C-budget-ua: Spend share on user acquisition
- kind: budget
- org: growth
- spend_share: 0.7

## C-gdpr: Privacy consent applies to UA changes
- kind: regulation
- applies_to: [ua.budget_shift]
- requires: gate
