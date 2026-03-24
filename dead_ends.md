# Dead Ends Log — CHPawn-FrozenKing
# Read before every milestone. Do NOT repeat these.

## DEAD END 0 — MCTS (pre-loaded)
**What**: Monte Carlo Tree Search — rollouts, UCB, backpropagation
**Why dead end**: Wrong algorithm. Frozen spec is minimax + alpha-beta.
**Do NOT write**: rollout, playout, UCB, visit_count, simulation_count,
                  backpropagate, MonteCarloNode, c_puct, num_simulations

## DEAD END 1 — Neural network evaluation (pre-loaded)
**What**: Any learned evaluation function
**Why dead end**: Evaluation is material + Michniewski PST only.
**Do NOT import**: tch, burn, candle, tract, onnxruntime, any ML crate

## DEAD END 2 — Wrong piece values (pre-loaded)
**What**: BISHOP=325, KNIGHT=320 from modern engine tuning
**Why dead end**: Frozen spec says BISHOP=300, KNIGHT=300.
**Do NOT write**: any piece value not in frozen/spec.md

## DEAD END 3 — Self-generated PST values (pre-loaded — HIGHEST RISK)
**What**: LLM generates its own piece-square table values
**Why dead end**: PST values are frozen in frozen/pst.rs from Michniewski.
                  Any deviation is prior contamination.
**Do NOT write**: any PST array not copied exactly from frozen/pst.rs
**Detection**: grep for any PST value not in the frozen file

## DEAD END 4 — Internal opening book (pre-loaded)
**What**: Polyglot book, hardcoded first moves, any book code
**Why dead end**: CCRL requires engines with internal books to disable them.
                  We have no book. CCRL provides the book externally.
**Do NOT write**: any book loading, book move selection, or hardcoded openings

## DEAD END 5 — Always-replace TT (pre-loaded)
**What**: Transposition table that always overwrites on collision
**Why dead end**: Depth+age hybrid replacement is frozen in spec (DD04).
                  Always-replace wastes deep search results.
**Do NOT write**: simple always-replace store() without priority calculation

## DEAD END 6 — Fixed quiescence depth (pre-loaded)
**What**: fn quiescence(pos, alpha, beta, depth: u8) with depth cap
**Why dead end**: Quiescence is unbounded per frozen spec.
**Do NOT add**: depth parameter to quiescence function
