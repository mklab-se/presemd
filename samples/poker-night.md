---
title: "Poker Night: Humans vs Machines"
author: "MDeck Demo"
date: 2026-02-28
@theme: dark
@transition: slide
---

# Poker Night: Humans vs Machines

A tale of bluffs, bots, and bad beats



![The table is set @fill](images/poker-4.png)

A smoke-filled saloon. Two players. One has a secret.



# Why Poker?

+ Incomplete information — you never see all the cards
+ Deception is a core mechanic, not a bug
+ Optimal play requires modeling your opponent
+ Machines had to learn to *lie* before they could win



# A Brief History

| Year | Milestone |
|------|-----------|
| 1970 | First computer poker programs |
| 1998 | Heads-up limit poker "solved" |
| 2017 | Libratus beats top pros at no-limit |
| 2019 | Pluribus defeats 5 human pros simultaneously |
| 2024 | AI poker bots flood online platforms |



![Reading the table @width:80%](images/poker-1.png)

The cowboy studies his hand. His opponent's cards glow with something... unnatural.



# The Human Advantage

- Intuition built from thousands of hands
- Reading physical tells — micro-expressions, breathing, posture
- Adapting strategy mid-session based on "feel"

+ But machines don't flinch.
+ They don't tilt after a bad beat.
+ And they never, ever get tired.



# The Machine Advantage

+ Game-theoretic optimal (GTO) play
+ Perfect recall of every hand ever played
+ No emotional leakage
+ Nanosecond decision-making

***

*"The bot doesn't need to read your face. It already knows the math."*



![The reveal @width:80%](images/poker-3.png)

Human hands and mechanical hands shuffle the same deck.



# How Poker AI Works

```@architecture
- Training Data  (icon: database,  pos: 1,1)
- Self-Play      (icon: function,  pos: 2,1)
- Strategy Net   (icon: network,   pos: 3,1)
- Live Game      (icon: user,      pos: 4,1)

+ Training Data -> Self-Play: billions of hands
+ Self-Play -> Strategy Net: learn equilibrium
+ Strategy Net -> Live Game: deploy
```



# Counterfactual Regret Minimization

The secret sauce behind modern poker AI:

```python
def cfr(game_state, player, reach_probs):
    if game_state.is_terminal():
        return game_state.payoff(player)

    strategy = get_strategy(game_state)
    action_values = {}

    for action in game_state.actions():
        next_state = game_state.apply(action)
        action_values[action] = cfr(next_state, player, updated_probs)

    node_value = sum(strategy[a] * action_values[a] for a in strategy)

    for action in game_state.actions():
        regret = action_values[action] - node_value
        cumulative_regret[action] += reach_probs * regret

    return node_value
```



![Four aces @width:80%](images/poker-2.png)

Sometimes you just get lucky. The AI knows exactly how often.



@layout: two-column

# Human vs Machine

**Strengths of Humans:**

- Creative bluffing
- Reading physical tells
- Emotional intelligence
- Adapting to "weird" players

+++

**Strengths of Machines:**

- Mathematical precision
- Tireless consistency
- Zero emotional bias
- Perfect memory



# What Poker Teaches Us About AI

+ AI doesn't need to be "smart" — it needs to be *strategic*
+ Deception isn't uniquely human
+ The best AI systems handle uncertainty, not just facts
+ Collaboration might beat competition



> In poker, as in life, the goal isn't to play the cards — it's to play the player.

-- Old saloon wisdom



# Thank You

Enjoy the game.
