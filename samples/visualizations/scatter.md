---
title: "Scatter Plot Tests"
@theme: dark
---

# Scatter Plot — Basic

```@scatter
# x-label: Hours Studied
# y-label: Test Score
- Alice: 80, 90
- Bob: 65, 75
- Carol: 90, 95
- Dave: 40, 60
- Eve: 70, 80
```


# Scatter Plot — With Sizes

```@scatter
# x-label: Revenue ($M)
# y-label: Growth (%)
- Startup A: 5, 120 (size: 20)
- Startup B: 15, 80 (size: 35)
- Corp C: 80, 15 (size: 50)
- Corp D: 120, 8 (size: 60)
- Mid E: 40, 45 (size: 40)
```


# Scatter Plot — No Labels

```@scatter
- Point 1: 10, 20
- Point 2: 30, 40
- Point 3: 50, 10
- Point 4: 20, 50
- Point 5: 60, 30
```


# Scatter Plot — Progressive Reveal

```@scatter
# x-label: X Axis
# y-label: Y Axis
- Static A: 10, 50
- Static B: 40, 30
+ Revealed C: 70, 80
+ Revealed D: 90, 20
* With E: 55, 65
```
