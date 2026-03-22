---
title: "Standardizing Our Workflow with Git Flow"
author: "Engineering Team"
@theme: dark
@transition: slide
---

# Standardizing Our Workflow with Git Flow

Reducing friction across teams  
A shared branching strategy for predictable delivery

???

Core message: This is not just an informational session — it’s about aligning on a decision to improve how we work together.

Talking points:
- Open by framing the problem space: multiple teams, shared codebase, increasing coordination overhead.
- Emphasize that this session is about **standardization**, not enforcing arbitrary rules.
- Highlight the goal: reduce friction, improve predictability, and make collaboration smoother.
- Make it clear that Git Flow is a *candidate solution*, not a foregone conclusion.

Delivery:
- Keep tone conversational and inclusive.
- Pause after stating the goal and ask: “Does this resonate with your current experience?”

Background:
- Teams without a shared workflow often accumulate invisible inefficiencies: merge conflicts, unclear ownership, and release anxiety.

Transition:
- “Let’s start by grounding this in what we’re experiencing today.”

---

# The Current Problem

+ Inconsistent branching strategies
+ Confusing merges and release processes
+ Cross-team coordination is painful

???

Core message: Our current workflow inconsistency is actively slowing us down and causing frustration.

Talking points:
- Start with inconsistency: different teams use different branching models (or none at all).
- Explain how this leads to confusion: “Where does this branch go?”, “What gets merged where?”
- Highlight coordination pain: teams stepping on each other, unclear ownership of releases.
- Use concrete examples if possible (merge conflicts, duplicated work, delayed releases).

Delivery:
- Reveal each point gradually and expand with a short example.
- Ask: “How many of you have had a confusing merge recently?”

Background:
- Lack of standardization scales poorly — what works for one team breaks across multiple teams.

Transition:
- “So what’s the impact of all this inconsistency?”

---

# Why This Matters

+ Slower delivery and more bugs
+ Harder onboarding for new developers
+ Lack of predictability in releases

???

Core message: These workflow issues directly impact both engineering efficiency and business outcomes.

Talking points:
- Slower delivery: time lost resolving conflicts, coordinating merges.
- More bugs: unstable branches, unclear release states.
- Onboarding pain: new developers must learn *multiple* workflows instead of one.
- Predictability: hard to answer “what’s going live and when?”

Delivery:
- Emphasize cause → effect relationships.
- Slightly slow down on “predictability” — it’s a key selling point.

Background:
- Predictability is critical for planning, QA, and stakeholder communication.

Transition:
- “Given these problems, what would a good solution actually look like?”

---

# What We Need

+ Clear, shared workflow
+ Defined branch responsibilities
+ Predictable release process

???

Core message: We need alignment on principles before jumping to solutions.

Talking points:
- Define success criteria:
  - Everyone understands the workflow.
  - Each branch has a clear purpose.
  - Releases follow a consistent, repeatable process.
- Emphasize: this is about reducing ambiguity.

Delivery:
- Present this as a checklist.
- Pause and let it sink in — this frames Git Flow evaluation.

Background:
- Without clear criteria, teams often adopt tools/processes that don’t actually solve their problems.

Transition:
- “Now let’s look at a model designed to meet these needs.”

---

# Enter Git Flow

+ A structured branching model
+ Designed for team collaboration
+ Separates development, release, and hotfix work

???

Core message: Git Flow provides structure and clarity through well-defined branch roles.

Talking points:
- Clarify: Git Flow is a *convention*, not a tool.
- It introduces structure where we currently have ambiguity.
- Key idea: separation of concerns — development vs release vs urgent fixes.

Delivery:
- Keep it high-level — avoid diving into details yet.
- Use emphasis on “structure” and “separation”.

Background:
- Git Flow is widely used in teams that need controlled releases and coordination.

Transition:
- “Let’s break down the core building blocks of Git Flow.”

---

# Core Branches

+ **main** → production-ready code
+ **develop** → integration branch for ongoing work

???

Core message: Git Flow revolves around two stable, clearly defined core branches.

Talking points:
- main:
  - Always production-ready
  - Represents what’s deployed
- develop:
  - Where ongoing work is integrated
  - Acts as the “working hub”
- Emphasize the separation between stability (main) and progress (develop).

Delivery:
- Slow down here — this is foundational.
- Reinforce: “Nothing unstable should go into main.”

Background:
- This separation reduces risk and improves confidence in releases.

Transition:
- “Beyond these, Git Flow introduces supporting branches for specific types of work.”

---

# Supporting Branches

+ **feature/*** → new work
+ **release/*** → stabilization before release
+ **hotfix/*** → urgent production fixes

???

Core message: Each type of work has a dedicated branch pattern, reducing ambiguity.

Talking points:
- feature branches:
  - Short-lived, branch from develop
  - Isolate new work
- release branches:
  - Prepare for release (testing, fixes)
- hotfix branches:
  - Critical fixes directly from main
- Emphasize: each branch type has a *clear purpose*.

Delivery:
- Use examples: “feature/login”, “release/1.0”.
- Keep energy up — this is where clarity becomes tangible.

Background:
- Clear branch roles reduce cognitive load and coordination overhead.

Transition:
- “Let’s see how all of this fits together in practice.”

---

# How Work Flows

```@gitgraph
- branch main
- branch develop from main
+ branch feature/login from develop
+ commit feature/login: "Build login feature"
+ merge feature/login -> develop: "PR merged"
+ branch release/1.0 from develop
+ commit release/1.0: "Stabilize release"
+ merge release/1.0 -> main: "Release 1.0"
+ branch hotfix/urgent-fix from main
+ commit hotfix/urgent-fix: "Fix production bug"
+ merge hotfix/urgent-fix -> main: "Apply hotfix"
* merge hotfix/urgent-fix -> develop
```

???

Core message: Git Flow provides a clear, repeatable lifecycle for all types of work.

Talking points:
- Walk step-by-step:
  1. Feature branches off develop
  2. Merge back into develop via PR
  3. Create release branch from develop
  4. Merge release into main for deployment
  5. Hotfix branches from main when needed
  6. Merge hotfix back into both main and develop
- Highlight how each step has a clear rule.

Delivery:
- Move slowly and narrate each step as it appears.
- This is the “aha” moment — don’t rush.

Background:
- The dual-merge for hotfixes ensures consistency across branches.

Transition:
- “So what does this actually improve for us?”

---

# What Improves

+ Clear ownership of work
+ Safer, more controlled releases
+ Reduced merge conflicts across teams

???

Core message: Git Flow directly addresses the pain points we identified earlier.

Talking points:
- Ownership: each branch type defines responsibility.
- Safer releases: release branches isolate stabilization work.
- Fewer conflicts: structured integration reduces chaos.

Delivery:
- Explicitly tie each benefit back to earlier problems.
- Example: “We said merges are confusing — this makes them predictable.”

Background:
- Structured workflows reduce both technical and communication overhead.

Transition:
- “Of course, this isn’t perfect — let’s look at the trade-offs.”

---

# Trade-offs

+ More structure = more discipline required
+ May feel heavier for small changes
+ Best suited for teams with regular releases

???

Core message: Git Flow has costs, and we should be realistic about them.

Talking points:
- More rules → requires team discipline.
- Overhead for small/simple changes.
- Works best when releases are frequent and coordinated.

Delivery:
- Be honest and neutral — this builds trust.
- Slight pause after each point to let it land.

Background:
- Alternatives (like trunk-based development) may be better for smaller or highly agile teams.

Transition:
- “So the real question is: does this fit *our* situation?”

---

# Is Git Flow Right for Us?

+ Multiple teams collaborating
+ Frequent releases and hotfixes
+ Need for predictability and stability

???

Core message: Git Flow aligns well with our current scale and needs.

Talking points:
- Map each point to your team reality:
  - Multiple teams → need coordination
  - Frequent releases → need structure
  - Predictability → current pain point
- Encourage the audience to evaluate honestly.

Delivery:
- Ask: “Do these conditions describe us?”
- Let people mentally agree before moving on.

Background:
- Adoption works best when the problem-solution fit is clear.

Transition:
- “If we agree this fits, here’s a concrete proposal.”

---

# Proposal

+ Adopt Git Flow as the standard workflow
+ Document conventions clearly
+ Run a short trial period

???

Core message: We are proposing a low-risk, structured adoption of Git Flow.

Talking points:
- Standardize on Git Flow across teams.
- Create clear documentation (branch naming, rules).
- Trial period:
  - Reduces risk
  - Allows feedback and iteration

Delivery:
- Emphasize “trial” — lowers resistance.
- Be clear this is not irreversible.

Background:
- Change management is easier when reversible and well-documented.

Transition:
- “If we move forward, here’s how we start.”

---

# Next Steps

+ Align on branch naming and rules
+ Provide quick onboarding guide
+ Revisit after trial for adjustments

???

Core message: The focus now is execution and momentum.

Talking points:
- Define conventions (naming, merge rules).
- Create lightweight onboarding material.
- Schedule a follow-up after the trial.

Delivery:
- End with energy and clarity.
- Reinforce that this is actionable, not theoretical.

Background:
- Successful adoption depends on clarity, documentation, and iteration.

Transition:
- “Let’s discuss — questions, concerns, or edge cases you’re thinking about?”