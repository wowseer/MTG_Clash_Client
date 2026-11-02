# Project Guidelines: MTG Arena-Style 3D Engine & Animations

## Core Objective
Develop a 3D Magic: The Gathering UI/Engine (Rust/Bevy or Three.js) that replicates the high-impact, fluid animation feel of MTG Arena.

## Animation & Motion Design Rules (Arena Style)

### 1. Motion Physics & Curves
- NEVER use linear transitions for UI or 3D Cards.
- ALWAYS use Elastic/Spring Physics or Snappy Cubic-Beziers:
  - Hover / Lift: `type: spring, stiffness: 300, damping: 22`
  - Cast / Play Card: `cubic-bezier(0.15, 0.85, 0.35, 1.2)` (Overshoot effect on landing)
  - Destruction / Graveyard: `cubic-bezier(0.55, 0, 1, 0.45)` (Fast acceleration down)

### 2. 3D Card Interactivity & States
- **Hover:** Card raises on Z-axis (+0.5 units), tilts slightly toward the mouse vector, and receives a soft edge-glow shader.
- **Dragging:** Card stays parallel to the camera plane with slight inertia/lag behind the cursor.
- **Spell Casting:** 
  1. **Wind-Up:** Card moves to center-screen, scales up 1.2x (300ms).
  2. **Resolution:** Flashes brightly, triggers impact VFX at target, shrinks and moves to Graveyard/Exile or Board.
- **Combat Lunge:** Attacking creature pulls back 10%, lunges forward rapidly to defender, triggers Screen-Shake, and spring-returns to position.

### 3. Visual Juice & FX Checklist
When writing code for actions (Tapping, Damage, Phase Transition, Spells):
- **Impact Feedback:** Include subtle screen/camera shake on high-damage or spell resolution.
- **Performance:** Keep animation logic on GPU-accelerated attributes (`transform`, `scale`, `rotation`, `opacity`).
- **Particle Systems:** Use short-lived, additive-blended particles for mana/casts (fade out < 500ms).
- **Staggering:** When multiple cards/triggers fire, apply a `0.06s` staggered delay between each event.

## Code Standards
- Keep Game State Transformations (Rules Engine) strictly separated from Visual/Animation Events.
- Event-Driven Architecture: The Rules Engine emits `GameEvent`s; the Animation/Render Loop listens and queues visual tweens.
