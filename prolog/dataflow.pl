:- encoding(utf8).
/** <module> The dataflow graph: constraints and derived values as nodes

(docs/design.md § Dataflow, § Claims and beliefs.) Loaded by server.pl into
the user module.

A node is defined by a fact node_def(Id, Def), asserted from the journal
like any fact, so it can be undone or retracted like any fact:

* constraint(Name, Vars, Body): a denial, `false :- Body`. Its value is the
  list of Vars-Provenance pairs for which Body holds. Each literal of Body
  that is not a built-in is matched against beliefs (belief(Seq)) and against
  claims (claim(Source, Seq)). A solution resting on beliefs alone is a
  violation; one that rests on a claim is a disagreement between sources.
* query(Vars, Goal): a derived value: the list of Vars for which Goal holds
  over beliefs only.

node/2 is tabled incrementally, so after a change SWI-Prolog recomputes only
the nodes whose facts changed (milestone 0's spike showed this per call
variant). Each evaluation bumps a counter, read by node_evals/2.

A fact is a belief or a claim: the server records every fact with its
journal seq in fact_ref/3; a claim is the fact claims(Source, P).
*/

:- dynamic([node_def/2, fact_ref/3], [incremental(true)]).
:- table node/2 as (incremental, shared).

node(Id, V) :-
    node_def(Id, Def),
    count(Id),
    eval(Def, V).

count(Id) :- eval_key(Id, K), flag(K, N, N + 1).
eval_key(Id, K) :- format(atom(K), "node_evals_~w", [Id]).

%!  node_evals(+Id, -N) is det.
%   How many times node Id has been evaluated.
node_evals(Id, N) :- eval_key(Id, K), flag(K, N, N).

eval(constraint(_Name, Vars, Body), V) :-
    findall(Vars-Prov, holds_all(Body, Prov), V0),
    sort(V0, V).
eval(query(Vars, Goal), V) :-
    findall(Vars, beliefs(Goal), V0),
    sort(V0, V).

%   A literal holds as a belief, or as some source's claim.
holds_all((A, B), P) :- !,
    holds_all(A, P1),
    holds_all(B, P2),
    append(P1, P2, P).
holds_all(G, []) :- builtin(G), !, call(G).
holds_all(G, [Who]) :- holds(G, Who).

holds(G, belief(S)) :- fact_ref(S, G, _).
holds(G, claim(Src, S)) :- fact_ref(S, claims(Src, G), _).

beliefs((A, B)) :- !, beliefs(A), beliefs(B).
beliefs(G) :- builtin(G), !, call(G).
beliefs(G) :- fact_ref(_, G, _).

builtin(G) :- predicate_property(system:G, built_in).

%!  plant_check(+Vars, +Body, -Result) is det.
%   Try a constraint against a planted violation (docs/design.md § Testing
%   claims, last row): give its variables distinct integers, in every order
%   until the built-ins hold, plant the other literals as facts, and run the
%   constraint over them. Result is fires(Planted) or broken(Why).
plant_check(Vars, Body, Result) :-
    copy_term(Vars-Body, _-B),
    term_variables(B, Vs),
    length(Vs, N),
    (   N > 7
    ->  Result = broken(too_many_variables(N))
    ;   numlist(1, N, Ns),
        (   permutation(Ns, P),
            copy_term(Vs-B, P-Ground),
            literals(Ground, Facts, Builtins),
            maplist(call, Builtins),
            planted_holds(Ground, Facts)
        ->  Result = fires(Facts)
        ;   Result = broken(no_planted_values_make_the_body_true)
        )
    ).

literals((A, B), F, Bs) :- !,
    literals(A, F1, B1), literals(B, F2, B2),
    append(F1, F2, F), append(B1, B2, Bs).
literals(G, [], [G]) :- builtin(G), !.
literals(G, [G], []).

%   Run the body with the planted facts as the only beliefs.
planted_holds((A, B), Fs) :- !, planted_holds(A, Fs), planted_holds(B, Fs).
planted_holds(G, _) :- builtin(G), !, call(G).
planted_holds(G, Fs) :- memberchk(G, Fs).
