---
title: "Git Flow: Should We Adopt It?"
author: "Engineering Team"
date: 2026-03-22
@theme: dark
@transition: slide
---

# Git Flow

A branching strategy for managing releases

???

Set the stage: this is an evaluation, not a decision meeting.  
Explain that the goal is shared understanding so we can decide together later.  
Keep it light and neutral—avoid biasing toward or against Git Flow yet.

---

# Why Branching Strategy Matters

- Controls how code moves to production  
+ Impacts collaboration, releases, and stability  

???

Explain that branching strategy is not just a Git detail—it shapes how the team works.  
Highlight any current friction: merge conflicts, unclear release process, inconsistent practices.  
Ask: “How confident are we in our current release flow?”

---

# What Is Git Flow?

- A structured branching model  
+ Defines specific branch types and roles  
+ Designed for release-based development  

???

Keep this high-level—don’t dive into mechanics yet.  
Frame Git Flow as an opinionated system, not just a suggestion.  
Emphasize: it’s built around releases, not continuous deployment.

---

# Core Branches

- **main**: production-ready code  
+ **develop**: integration branch for features  

???

Explain the relationship clearly:  
- main = what’s live  
- develop = where features accumulate  

Use a simple mental model: develop is the “next release,” main is “current production.”

---

# Supporting Branches

- **feature/***: new work  
+ **release/***: prepare releases  
+ **hotfix/***: urgent production fixes  

???

Walk through each briefly:  
- feature branches isolate work  
- release branches stabilize before shipping  
- hotfix branches bypass normal flow for emergencies  

Emphasize purpose, not mechanics.

---

# How It Works (Flow)

```@architecture
# Components
- main     (pos: 1,2)
- develop  (pos: 2,2)
- feature  (pos: 2,1)
- release  (pos: 3,1)
- hotfix   (pos: 1,1)

# Relationships
- main -- develop: long-lived branches
+ develop -> feature: branch for work
+ feature -> develop: merge completed work
+ develop -> release: prepare release
+ release -> main: release to production
* release -> develop: sync back
+ main -> hotfix: urgent fix
* hotfix -> main: patch production
* hotfix -> develop: keep in sync
```

???

Walk through step-by-step, not all at once.  
Use the progressive reveal to explain flow incrementally.  
Narrate like a lifecycle: feature → develop → release → main → hotfix.  
Pause after each step to ensure understanding.

---

# Example Lifecycle

```@timeline
- Start: develop branch active
+ Feature Work: feature/login created and merged
+ Release Prep: release/v1.2 created
+ Stabilization: bug fixes on release branch
+ Production Release: merged to main + tagged v1.2
+ Post-Release: merged back into develop
```

???

Make it concrete with a relatable example (e.g., login feature).  
Show how code moves through stages.  
Emphasize the “release stabilization” phase—this is key to Git Flow.  
Ask: “Do we currently have a clear stabilization phase?”

---

# What Git Flow Is NOT

- Not optimized for continuous deployment  
+ Not minimal or lightweight  
+ Not ideal for very small teams  

???

Reset expectations—this avoids misuse.  
Be explicit: Git Flow adds structure and process overhead.  
Contrast with modern CI/CD practices.

---

# Benefits

- Clear structure and roles  
+ Safer release process  
+ Parallel work without blocking  

???

Tie each benefit to real scenarios:  
- fewer release surprises  
- teams working independently  
- predictable release cycles  

Avoid overselling—keep it grounded.

---

# Trade-offs

- More complexity and overhead  
+ Long-lived branches can drift  
+ Slower integration cycles  

???

Be honest here—this builds credibility.  
Explain “branch drift” and integration pain.  
Ask: “Would this slow us down given how we work today?”

---

# When Git Flow Works Well

- Scheduled releases  
+ Multiple versions in support  
+ Teams needing strict release control  

???

Help the audience self-identify.  
Give examples: enterprise products, versioned APIs.  
Ask: “Do we operate like this?”

---

# When It Might Not Fit

- Continuous deployment environments  
+ High-frequency releases  
+ Teams favoring trunk-based development  

???

Contrast with modern practices.  
Mention that many high-performing teams use simpler flows.  
Frame this as a trade-off, not a flaw.

---

# Alternatives

- Trunk-based development  
+ GitHub Flow  
+ GitLab Flow  

???

Position Git Flow in context.  
Briefly describe each alternative at a high level.  
Avoid deep comparison—just show it’s not the only option.

---

# Adopting Git Flow (If We Choose To)

- Define branch rules and naming  
+ Automate with tooling  
+ Align CI/CD with flow  

???

Focus on practicality.  
Mention tools like Git hooks, CI pipelines.  
Stress that process + tooling must align.

---

# Team Implications

- Requires discipline and consistency  
+ Clear ownership of releases  
+ Shared understanding is critical  

???

Emphasize this is a team contract, not just a Git setup.  
Highlight risk: inconsistency breaks the model.  
Ask: “Are we ready to commit to this level of discipline?”

---

# Questions to Decide

- Do we need structured releases?  
+ Can we handle added complexity?  
+ Does it match our deployment model?  

???

Drive discussion.  
Pause after each question—let people think or respond.  
Encourage honest answers, not consensus pressure.

---

# Summary

- Git Flow = structured but heavier workflow  
+ Great for controlled releases  
+ Not ideal for fast CI/CD teams  

???

Reinforce a balanced view.  
Avoid framing it as “good” or “bad”—it’s context-dependent.  
Restate the goal: informed decision-making.

---

# Next Steps

- Discuss fit for our team  
+ Option: trial on a project  
+ Decide: adopt, adapt, or reject  

???

End with action.  
Suggest a low-risk experiment if appropriate.  
Encourage follow-up discussion rather than immediate decision.