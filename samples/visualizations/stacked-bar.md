---
title: "Stacked Bar Chart Tests"
@theme: dark
---

# Stacked Bar — Basic

```@stackedbar
# categories: Q1, Q2, Q3, Q4
- Product A: 40, 45, 50, 55
- Product B: 30, 35, 40, 45
```


# Stacked Bar — With Axis Labels

```@stackedbar
# categories: Q1, Q2, Q3, Q4
# x-label: Quarter
# y-label: Revenue ($M)
- Product A: 40, 45, 50, 55
- Product B: 30, 35, 40, 45
- Product C: 15, 20, 25, 30
```


# Stacked Bar — Progressive Reveal

```@stackedbar
# categories: 2022, 2023, 2024
# y-label: Headcount
- Engineering: 50, 80, 120
+ Sales: 30, 45, 60
* Marketing: 20, 30, 40
+ Support: 15, 25, 35
```


# Stacked Bar — Many Series

```@stackedbar
# categories: US, EU, APAC, LATAM
# x-label: Region
# y-label: Revenue ($M)
- SaaS: 100, 80, 60, 30
- Consulting: 40, 50, 30, 20
- Hardware: 20, 15, 40, 10
- Support: 30, 25, 20, 15
```
