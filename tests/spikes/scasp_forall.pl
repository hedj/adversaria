% Spike, milestone 0: s(CASP) issue #13 (github.com/SWI-Prolog/sCASP/issues/13).
% bad holds, through p(a,x) and q(a,x), so the query `not bad` must have no model.
% Fault caught: with the default --forall algorithm, s(CASP) 1.1.4 is expected
% to check only some values of Y and wrongly prove `not bad` (docs/translator.md).
p(a, x).
p(b, y).
q(a, x).
bad :- p(X, Y), q(X, Y).

?- not bad.
