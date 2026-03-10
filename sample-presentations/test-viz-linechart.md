---
title: "Line Chart Tests"
@theme: dark
---

# Line Chart — Single Series

```@linechart
# x-labels: Q1, Q2, Q3, Q4
- Revenue: 100, 150, 200, 280
```


# Line Chart — With Axis Labels

```@linechart
# x-labels: Jan, Feb, Mar, Apr, May, Jun
# x-label: Month
# y-label: Temperature (°C)
- London: 5, 6, 10, 14, 17, 20
- Madrid: 10, 12, 16, 19, 23, 28
```


# Line Chart — Multiple Series

```@linechart
# x-labels: 2020, 2021, 2022, 2023, 2024
# x-label: Year
# y-label: Users (millions)
- Product A: 10, 25, 45, 80, 120
+ Product B: 5, 15, 30, 55, 90
+ Product C: 2, 8, 20, 40, 70
```


# Line Chart — Progressive Reveal

```@linechart
# x-labels: Mon, Tue, Wed, Thu, Fri
# y-label: Requests (k)
- API v1: 120, 115, 130, 125, 140
+ API v2: 80, 95, 110, 130, 160
* Legacy: 40, 35, 30, 25, 20
```
