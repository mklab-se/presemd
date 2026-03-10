---
title: "Org Chart Tests"
@theme: dark
---

# Org Chart — Basic

```@orgchart
- CEO
- CEO -> CTO
- CEO -> CFO
- CEO -> COO
- CTO -> VP Engineering
- CTO -> VP Product
- CFO -> Controller
```

---

# Org Chart — Deep Hierarchy

```@orgchart
- CEO
- CEO -> CTO
- CEO -> CMO
- CEO -> CFO
- CTO -> VP Engineering
- CTO -> VP Infrastructure
- VP Engineering -> Frontend Lead
- VP Engineering -> Backend Lead
- VP Engineering -> Mobile Lead
- Frontend Lead -> Senior Dev
- Backend Lead -> Platform Team
- VP Infrastructure -> DevOps Lead
- VP Infrastructure -> Security Lead
```

---

# Org Chart — Progressive Reveal

```@orgchart
- Director
+ Director -> Team Lead A
+ Director -> Team Lead B
* Team Lead A -> Dev 1
* Team Lead A -> Dev 2
+ Team Lead B -> Dev 3
```
