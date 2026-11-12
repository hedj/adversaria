:- encoding(utf8).
% Spike, milestone 0: does SWI-Prolog's incremental tabling recompute
% only the tabled answers that a changed fact invalidates?
% Fault caught: the design (§ Dataflow) assumes it does. If every table is
% recomputed, or a stale answer survives, the dataflow engine must be built by hand.

:- dynamic([a/1, b/1, c/2], [incremental(true)]).
:- table p/1 as incremental, q/1 as incremental, r/2 as incremental,
         s/1 as incremental.

a(1). a(2).
b(10). b(20).
c(k1, 1). c(k2, 2).

% Each evaluation of a tabled body bumps a counter, so recomputation shows.
p(X)    :- flag(p_evals, N, N+1), a(X).
q(X)    :- flag(q_evals, N, N+1), b(X).
r(K, X) :- atom_concat(r_evals_, K, F), flag(F, N, N+1), c(K, X).  % one table per key
% (flag/3 keys a compound by name and arity only, so the key must be an atom.)
s(X)    :- flag(s_evals, N, N+1), p(X), X > 1.  % depends on p, not on a directly

answers(G, L) :- findall(X, call(G, X), L0), msort(L0, L).
ev(Key, N) :- flag(Key, N, N).

report(Step) :-
    answers(p, Ps), answers(q, Qs), answers(s, Ss),
    answers(r(k1), R1), answers(r(k2), R2),
    ev(p_evals, PE), ev(q_evals, QE), ev(s_evals, SE),
    ev(r_evals_k1, R1E), ev(r_evals_k2, R2E),
    format("~w~t~24|p=~w q=~w s=~w r(k1)=~w r(k2)=~w~n", [Step, Ps, Qs, Ss, R1, R2]),
    format("~t~24|evals: p=~w q=~w s=~w r(k1)=~w r(k2)=~w~n", [PE, QE, SE, R1E, R2E]).

check(Goal) :- ( call(Goal) -> true ; format("FAILED: ~q~n", [Goal]), halt(1) ).

main :-
    report('first query'),
    report('repeat query'),
    assertz(a(3)),       report('assertz(a(3))'),
    retract(b(10)),      report('retract(b(10))'),
    assertz(c(k1, 9)),   report('assertz(c(k1,9))'),
    check(answers(p, [1,2,3])), check(answers(q, [20])), check(answers(s, [2,3])),
    check(answers(r(k1), [1,9])), check(answers(r(k2), [2])),
    % Only the invalidated tables were re-evaluated, each once.
    check(ev(p_evals, 2)), check(ev(q_evals, 2)), check(ev(s_evals, 2)),
    check(ev(r_evals_k1, 2)), check(ev(r_evals_k2, 1)),
    writeln('answers correct; only invalidated tables recomputed').
