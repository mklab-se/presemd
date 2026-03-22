---
title: "Git Graph Visualization"
@theme: dark
@transition: slide
---

# Git Graph — Basic

```@gitgraph
- branch main
- branch develop from main
- branch feature/login from develop
- merge feature/login -> develop
- merge develop -> main: "v1.0"
```

---

# Git Graph — Progressive Reveal

```@gitgraph
- branch main
+ branch develop from main
+ branch feature/auth from develop
+ commit feature/auth: "Add OAuth"
+ commit feature/auth: "Add tests"
+ merge feature/auth -> develop: "PR #12"
+ merge develop -> main: "Release v2.0"
```

---

# Git Flow

```@gitgraph
- branch main
- branch develop from main
+ branch feature/login from develop
+ commit feature/login: "Login form"
+ merge feature/login -> develop
+ branch feature/api from develop
+ commit feature/api: "REST endpoints"
+ merge feature/api -> develop
+ branch release/1.0 from develop
+ commit release/1.0: "Bump version"
+ merge release/1.0 -> main: "v1.0"
* merge release/1.0 -> develop
```

---

# Hotfix Flow

```@gitgraph
- branch main
- branch develop from main
- commit main: "v1.0"
+ branch hotfix/crash from main
+ commit hotfix/crash: "Fix null pointer"
+ merge hotfix/crash -> main: "v1.0.1"
* merge hotfix/crash -> develop
```

---

# Multiple Features

```@gitgraph
- branch main
- branch develop from main
+ branch feature/ui from develop
* branch feature/api from develop
+ commit feature/ui: "New dashboard"
+ commit feature/api: "GraphQL layer"
+ merge feature/ui -> develop: "PR #1"
+ merge feature/api -> develop: "PR #2"
+ branch release/2.0 from develop
+ merge release/2.0 -> main: "v2.0"
* merge release/2.0 -> develop
```
