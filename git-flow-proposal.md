---
title: "Git Flow: A Smarter Way to Work Together"
author: "Team Presentation"
@theme: dark
@transition: slide
---

# Git Flow

A structured branching strategy for teams

+ Goal: consistency  
+ Fewer conflicts  
+ Smoother releases

???

Core message: This presentation is about improving how the team collaborates, not just learning Git commands.

Talking points:
- Open by framing this as a *team productivity and reliability* discussion.
- Emphasize that Git Flow is not new or experimental—it’s a widely used, proven workflow.
- Highlight the goal: reduce friction, confusion, and last-minute chaos.
- Briefly preview what’s coming: problems today, what Git Flow is, how it works, and whether we should adopt it.

Delivery:
- Keep it conversational and relatable.
- Pause after listing goals and ask: “Does this sound like something we need?”

Context:
- Many teams struggle not because of lack of skill, but lack of shared structure.

Transition:
- Move into the current pain points the team likely experiences.

---

# The Problem Today

+ No shared workflow  
+ Inconsistent branching & merging  
+ Harder releases, more conflicts  

???

Core message: Our current lack of structure is causing real, recurring problems.

Talking points:
- Describe what “no shared workflow” looks like in practice:
  - Everyone creates branches differently
  - No agreement on when or how to merge
- Talk about inconsistent merging:
  - Some merge to main, some to develop (if it exists), some rebase, some don’t
- Highlight release pain:
  - Last-minute fixes
  - Unexpected bugs
  - Merge conflicts piling up right before deployment

Delivery:
- Make this concrete—reference real situations the team has experienced.
- Slight pause after each bullet to let it sink in.

Context:
- These issues scale with team size—what worked for 2 people breaks at 5+.

Transition:
- Introduce Git Flow as a structured solution to these exact problems.

---

# What Is Git Flow?

+ A defined branching model  
+ Separates development, releases, and hotfixes  
+ Designed for team collaboration  

???

Core message: Git Flow provides structure and removes ambiguity.

Talking points:
- Define Git Flow simply: a *set of rules for how we use branches*.
- Emphasize separation:
  - Development work is isolated from production
  - Releases are controlled
  - Hotfixes are handled safely
- Stress that this is about *predictability*.

Delivery:
- Keep it high-level—don’t dive into details yet.
- Use phrases like “clear roles for each branch.”

Context:
- Without structure, teams rely on habits; Git Flow replaces habits with shared rules.

Transition:
- Now explain the core building blocks: the main branches.

---

# Core Branches

+ **main** → production-ready code  
+ **develop** → integration branch for features  

???

Core message: Everything in Git Flow revolves around two primary branches.

Talking points:
- Explain **main**:
  - Always stable
  - Always deployable
  - Represents production
- Explain **develop**:
  - Where all feature work comes together
  - Not necessarily stable at every moment
- Reinforce: *No direct feature work goes into main.*

Delivery:
- Emphasize “main must always be safe.”
- Pause and ask: “How often is our main branch actually stable today?”

Context:
- This separation is what enables safe releases.

Transition:
- Introduce the supporting branches that handle actual work.

---

# Supporting Branch Types

+ **feature** → new work  
+ **release** → prep for deployment  
+ **hotfix** → urgent production fixes  

???

Core message: Each type of work has a dedicated branch, eliminating confusion.

Talking points:
- Feature branches:
  - Created from develop
  - Used for isolated work
- Release branches:
  - Stabilize code before deployment
  - Bug fixes only, no new features
- Hotfix branches:
  - Created from main
  - Fix production issues immediately

Delivery:
- Clarify that naming matters—it communicates intent.
- Emphasize “one purpose per branch type.”

Context:
- This clarity is what prevents messy, mixed-purpose branches.

Transition:
- Show how all these pieces come together in a real workflow.

---

# How Work Flows

```@gitgraph
- branch main
- branch develop from main
+ branch feature/login from develop
+ commit feature/login: "build login"
+ merge feature/login -> develop: "feature complete"
+ branch release/1.0 from develop
+ merge release/1.0 -> main: "release 1.0"
* merge release/1.0 -> develop: "sync back"
```

???

Core message: This is the lifecycle of a feature from start to production.

Talking points:
- Start with feature branching from develop
- Work happens in isolation (commits)
- Merge back into develop when complete
- Create a release branch to stabilize
- Merge release into main for deployment
- Sync back to develop

Delivery:
- Step through each reveal slowly.
- Narrate it like a story: “We start here… then this happens…”
- Pause after release step—this is a key concept.

Context:
- The “sync back” step prevents divergence between branches.

Transition:
- Now explain how urgent production fixes are handled.

---

# Handling Hotfixes

```@gitgraph
- branch main
- branch develop from main
+ branch hotfix/critical from main
+ commit hotfix/critical: "fix bug"
+ merge hotfix/critical -> main: "patch"
* merge hotfix/critical -> develop: "sync fix"
```

???

Core message: Hotfixes allow fast, safe fixes without disrupting ongoing work.

Talking points:
- Hotfix starts from main (production state)
- Fix is applied immediately
- Merged back into main for quick deployment
- Then merged into develop to keep everything aligned

Delivery:
- Emphasize speed and safety.
- Ask: “What do we do today when production breaks?”

Context:
- Without this, teams often patch inconsistently or forget to sync fixes.

Transition:
- Summarize why this structured approach works so well.

---

# Why This Works

+ Clear separation of concerns  
+ Parallel work without collisions  
+ Predictable release cycles  

???

Core message: Structure leads directly to better outcomes.

Talking points:
- Separation reduces risk
- Parallel work avoids blocking teammates
- Predictable releases reduce stress and surprises

Delivery:
- Tie each point back to earlier problems.
- Slight emphasis on “predictable”—this is a major benefit.

Context:
- This is about reducing cognitive load across the team.

Transition:
- Bring it closer to home—what this means for us specifically.

---

# Impact on Our Team

+ Fewer merge conflicts  
+ Safer releases  
+ Shared mental model  

???

Core message: This will make our day-to-day work easier and more consistent.

Talking points:
- Fewer conflicts:
  - Smaller, isolated changes
- Safer releases:
  - Tested and stabilized before deployment
- Shared mental model:
  - Everyone knows what each branch means

Delivery:
- Make it personal: “This affects your daily work.”
- Pause and let the benefits resonate.

Context:
- Consistency is often more valuable than flexibility in teams.

Transition:
- Show how we can adopt this without disruption.

---

# Adoption Plan

+ Start with new features  
+ Define branch naming rules  
+ Align on PR & merge process  

???

Core message: We can adopt Git Flow incrementally and safely.

Talking points:
- No need for a big migration
- Start applying it to new work only
- Agree on naming conventions (feature/, release/, hotfix/)
- Standardize PR reviews and merge strategy

Delivery:
- Emphasize low risk and gradual change.
- Avoid overwhelming the audience.

Context:
- Adoption fails when it feels too heavy—keep it simple.

Transition:
- Close with a clear decision point.

---

# Decision Time

+ Do we want consistency?  
+ Do we want smoother releases?  
+ Adopt Git Flow?  

???

Core message: This is a team decision about improving how we work.

Talking points:
- Reframe Git Flow as a solution to existing problems
- Highlight benefits again briefly
- Invite discussion and feedback

Delivery:
- Pause after each question.
- Let the room think before speaking again.

Context:
- Adoption works best when the team feels ownership.

Transition:
- Open the floor for questions or discussion.