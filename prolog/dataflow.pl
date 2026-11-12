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
%   Filled by server.pl once it is loaded: the predicates that are not the
%   user's facts.
:- dynamic(server_predicate/1).
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
%   A layer of a view: each solution's mark, with the seqs of the beliefs
%   it rests on, through rules (crates/memex/src/view.rs).
eval(layer(Mark, Goal), V) :-
    findall(Mark-Seqs, (prove(Goal, P), sort(P, Seqs)), V0),
    sort(V0, V).

%   Prove a goal from beliefs, as beliefs/1 does, collecting the seqs of
%   the facts used, looking through rules to the facts under them.
prove((A, B), P) :- !, prove(A, P1), prove(B, P2), append(P1, P2, P).
prove(G, []) :- builtin(G), !, safe_call(G).
prove(G, [S]) :- fact_ref(S, G, _).
prove(G, P) :- fact_ref(_, rule_def(_, G, B), _), prove(B, P).

%   A literal holds as a belief, or as some source's claim.
holds_all((A, B), P) :- !,
    holds_all(A, P1),
    holds_all(B, P2),
    append(P1, P2, P).
holds_all(G, []) :- builtin(G), !, safe_call(G).
holds_all(G, [Who]) :- holds(G, Who).
%   Through a rule, a solution rests on the facts under it (and their
%   sources), not on the rule: those are what the user would change.
holds_all(G, P) :- fact_ref(_, rule_def(_, G, B), _), holds_all(B, P).

%   Provenance names the fact too: belief(Seq, Fact), claim(Source, Seq,
%   Fact). A Wikidata claim is derived from the imported subset, through the
%   user's links (same_as) and property mappings (wd_property).
holds(G, belief(S, G)) :- fact_ref(S, G, _).
holds(G, claim(Src, S, G)) :- fact_ref(S, claims(Src, G), _).
holds(G, claim("Wikidata", S, G)) :- wd_import(S, _), wd_claim(G).

beliefs((A, B)) :- !, beliefs(A), beliefs(B).
beliefs(G) :- builtin(G), !, safe_call(G).
beliefs(G) :- fact_ref(_, G, _).
beliefs(G) :- fact_ref(_, rule_def(_, G, B), _), beliefs(B).

builtin(G) :- predicate_property(system:G, built_in).

		 /*******************************
		 *     WHAT A DEFINITION MAY CALL *
		 *******************************/

%   A constraint's body, a derived value's goal and a rule's body are read
%   by the reasoner, so they must only read: built-ins are called, and
%   shell/1, open/3, assertz/1 and halt/0 are built-ins too. A built-in is
%   called only if it is on the list below (comparison, arithmetic, type
%   tests, term, atom and string operations, sorting), or is negation,
%   disjunction, if-then-else, findall/3,4 or forall/2 whose subgoals all
%   pass the same test. Inside those, a goal that is not a built-in must be
%   one of the user's fact predicates, not one of this server's own.

%!  safe_call(:Goal) is nondet.
%   Goal, if vet/1 allows it; else a permission error naming what is not
%   allowed. Negation, disjunction, if-then-else, findall and forall are
%   interpreted here, their subgoals proved by beliefs/1, so a rule works
%   inside them as it does outside (calling a rule's head directly would
%   find no such predicate: rules are rule_def facts).
safe_call(G) :- vet(G), control(G).

control((C -> T ; E)) :- !, ( beliefs(C) -> beliefs(T) ; beliefs(E) ).
control((C *-> T ; E)) :- !, ( beliefs(C) *-> beliefs(T) ; beliefs(E) ).
control((A ; B)) :- !, ( beliefs(A) ; beliefs(B) ).
control((C -> T)) :- !, ( beliefs(C) -> beliefs(T) ).
control((C *-> T)) :- !, ( beliefs(C) *-> beliefs(T) ).
control(\+ A) :- !, \+ beliefs(A).
control(findall(T, G, L)) :- !, findall(T, beliefs(G), L).
control(findall(T, G, L, R)) :- !, findall(T, beliefs(G), L, R).
control(forall(C, A)) :- !, forall(beliefs(C), beliefs(A)).
control(G) :- call(G).

%   The planted check calls built-ins directly: it tests a constraint
%   against planted facts alone, not against the world's beliefs.
planted_call(G) :- vet(G), call(G).

%!  vet(+Goal) is det.
%   Throw error(permission_error(call, not_allowed, What), _) unless Goal
%   may be called by the reasoner.
vet(G) :- var(G), !, not_allowed(variable_goal).
vet((A, B)) :- !, vet(A), vet(B).
vet((A ; B)) :- !, vet(A), vet(B).
vet((A -> B)) :- !, vet(A), vet(B).
vet((A *-> B)) :- !, vet(A), vet(B).
vet(\+ A) :- !, vet(A).
vet(findall(_, G, _)) :- !, vet(G).
vet(findall(_, G, _, _)) :- !, vet(G).
vet(forall(C, A)) :- !, vet(C), vet(A).
vet(G) :- builtin(G), !, functor(G, N, A), ( safe_builtin(N, A) -> true ; not_allowed(N/A) ).
vet(G) :- callable(G), \+ own_predicate(G), !.
vet(G) :- callable(G), !, functor(G, N, A), not_allowed(N/A).
vet(G) :- not_allowed(G).

not_allowed(What) :- throw(error(permission_error(call, not_allowed, What), _)).

%   A predicate this server's files define (node/2, load_wikidata/2,
%   fact_ref/3, ...) or a library's: not the user's facts.
own_predicate(G) :- functor(G, N, A), server_predicate(N/A), !.
own_predicate(G) :- predicate_property(user:G, file(_)), !.
own_predicate(G) :- predicate_property(user:G, imported_from(_)).

%!  check_rule(+Head, +Body) is det.
%   For the memex, before it keeps a rule Head :- Body: the body may only
%   call what a definition may (vet/1); the head is not a built-in or one
%   of this server's predicates; every variable of the head is bound by the
%   body (else the rule would answer with unbound values); and the rule is
%   not recursive, directly or through other rules, as rules are proved by
%   plain recursion, which would not end.
check_rule(Head, Body) :-
    vet(Body),
    (   callable(Head), \+ builtin(Head), \+ own_predicate(Head)
    ->  true
    ;   functor(Head, N, A), not_allowed(N/A)
    ),
    term_variables(Head, HVs), term_variables(Body, BVs),
    (   member(V, HVs), \+ ( member(W, BVs), W == V )
    ->  throw(error(domain_error(head_variable_bound_by_body, V), _))
    ;   true
    ),
    functor(Head, N, A),
    (   reaches(Body, N/A, [])
    ->  throw(error(permission_error(define, recursive_rule, N/A), _))
    ;   true
    ).

%   Body calls N/A, itself or through the rules of what it calls.
reaches(Body, PI, Seen) :-
    body_literal(Body, L),
    callable(L),
    functor(L, LN, LA),
    (   LN/LA == PI
    ->  true
    ;   \+ memberchk(LN/LA, Seen),
        functor(H, LN, LA),
        fact_ref(_, rule_def(_, H, B), _),
        reaches(B, PI, [LN/LA|Seen])
    ),
    !.

body_literal(B, _) :- var(B), !, fail.
body_literal((A, B), L) :- !, ( body_literal(A, L) ; body_literal(B, L) ).
body_literal((A ; B), L) :- !, ( body_literal(A, L) ; body_literal(B, L) ).
body_literal((A -> B), L) :- !, ( body_literal(A, L) ; body_literal(B, L) ).
body_literal((A *-> B), L) :- !, ( body_literal(A, L) ; body_literal(B, L) ).
body_literal(\+ A, L) :- !, body_literal(A, L).
body_literal(findall(_, G, _), L) :- !, body_literal(G, L).
body_literal(findall(_, G, _, _), L) :- !, body_literal(G, L).
body_literal(forall(C, A), L) :- !, ( body_literal(C, L) ; body_literal(A, L) ).
body_literal(L, L).

%!  check_definition(+Body) is det.
%   For the memex, before it keeps a constraint or derived value: vet/1,
%   so a refused definition never enters the journal.
check_definition(Body) :- vet(Body).

safe_builtin(N, A) :- safe_builtin_(N, A), !.
safe_builtin_(=, 2).      safe_builtin_(\=, 2).     safe_builtin_(==, 2).
safe_builtin_(\==, 2).    safe_builtin_(@<, 2).      safe_builtin_(@>, 2).
safe_builtin_(@=<, 2).    safe_builtin_(@>=, 2).     safe_builtin_(compare, 3).
safe_builtin_(=@=, 2).    safe_builtin_(\=@=, 2).
safe_builtin_(is, 2).     safe_builtin_(=:=, 2).     safe_builtin_(=\=, 2).
safe_builtin_(<, 2).      safe_builtin_(>, 2).       safe_builtin_(=<, 2).
safe_builtin_(>=, 2).     safe_builtin_(succ, 2).    safe_builtin_(plus, 3).
safe_builtin_(between, 3).
safe_builtin_(true, 0).   safe_builtin_(fail, 0).    safe_builtin_(false, 0).
safe_builtin_(var, 1).    safe_builtin_(nonvar, 1).  safe_builtin_(atom, 1).
safe_builtin_(number, 1). safe_builtin_(integer, 1). safe_builtin_(float, 1).
safe_builtin_(string, 1). safe_builtin_(atomic, 1).  safe_builtin_(compound, 1).
safe_builtin_(callable, 1). safe_builtin_(is_list, 1). safe_builtin_(ground, 1).
safe_builtin_(functor, 3). safe_builtin_(arg, 3).    safe_builtin_(=.., 2).
safe_builtin_(copy_term, 2).
safe_builtin_(atom_length, 2). safe_builtin_(atom_concat, 3). safe_builtin_(sub_atom, 5).
safe_builtin_(atom_chars, 2).  safe_builtin_(atom_codes, 2).  safe_builtin_(char_code, 2).
safe_builtin_(atom_number, 2). safe_builtin_(atom_string, 2). safe_builtin_(number_codes, 2).
safe_builtin_(number_string, 2). safe_builtin_(upcase_atom, 2). safe_builtin_(downcase_atom, 2).
safe_builtin_(atomic_list_concat, 2). safe_builtin_(atomic_list_concat, 3).
safe_builtin_(string_concat, 3). safe_builtin_(string_length, 2). safe_builtin_(sub_string, 5).
safe_builtin_(string_chars, 2). safe_builtin_(string_codes, 2). safe_builtin_(string_code, 3).
safe_builtin_(string_lower, 2). safe_builtin_(string_upper, 2). safe_builtin_(split_string, 4).
safe_builtin_(length, 2). safe_builtin_(msort, 2). safe_builtin_(sort, 2).
safe_builtin_(sort, 4).   safe_builtin_(keysort, 2). safe_builtin_(memberchk, 2).

%!  plant_check(+Vars, +Body, -Result) is det.
%   Try a constraint against a planted violation (docs/design.md § Testing
%   claims, last row): give its variables distinct integers (1..N, then
%   -N..-1, 0..N-1, -(N-1)..0 and -1, 1, -2, 2, ...), in every order
%   until the built-ins hold, plant the other literals as facts, and run
%   the constraint over them. Result is fires(Planted) or broken(Why).
plant_check(Vars, Body, Result) :-
    copy_term(Vars-Body, _-B),
    term_variables(B, Vs),
    length(Vs, N),
    (   N > 7
    ->  Result = broken(too_many_variables(N))
    ;   M is N - 1, Lo is -N, Hi is -1, Lo1 is -M,
        numlist(1, N, Pos), numlist(Lo, Hi, Neg), numlist(0, M, Zero), numlist(Lo1, 0, NegZero),
        findall(V, (between(1, N, K), V is (K + 1) // 2 * (-1) ** K), Alt),
        % Distinct values from 1 up first; then negative ones, ranges
        % holding zero, and alternating signs (-1, 1, -2, 2, ...), for
        % bodies such as L < 0, X =< 0, or A < 0, B > 0.
        (   member(Ns, [Pos, Neg, Zero, NegZero, Alt]),
            permutation(Ns, P),
            copy_term(Vs-B, P-Ground),
            literals(Ground, Facts, Builtins),
            maplist(planted_call, Builtins),
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
planted_holds(G, _) :- builtin(G), !, planted_call(G).
planted_holds(G, Fs) :- memberchk(G, Fs).


		 /*******************************
		 *          WIKIDATA            *
		 *******************************/

%   A subset of Wikidata, loaded from a file of facts made by the wikidata
%   tool: wd(Item, Property, Value), wd_label(Entity, Label) and
%   wd_constraint(Property, Kind). One import at a time.
:- dynamic([wd/3, wd_label/2, wd_constraint/2, wd_import/2], [incremental(true)]).

load_wikidata(File, Seq) :-
    unload_wikidata,
    read_wikidata(File, assertz),
    assertz(wd_import(Seq, File)).

%!  check_wikidata(+File) is det.
%   Read File as load_wikidata/2 would, keeping nothing; throw on the first
%   term that is not a Wikidata fact. The memex calls it before it writes
%   an import, so a file it could not load never enters the journal.
check_wikidata(File) :-
    read_wikidata(File, [_]>>true).

read_wikidata(File, Keep) :-
    setup_call_cleanup(open(File, read, In, [encoding(utf8)]),
                       read_terms(In, Keep),
                       close(In)).

read_terms(In, Keep) :-
    read_term(In, T, []),
    (   T == end_of_file
    ->  true
    ;   (   wikidata_fact(T)
        ->  call(Keep, T)
        ;   numbervars(T, 0, _),
            format(string(S), "~W", [T, [max_depth(6), numbervars(true), quoted(true)]]),
            throw(error(domain_error(wikidata_fact, S), _))
        ),
        read_terms(In, Keep)
    ).

%   The facts the wikidata tool writes, and only those: ground, with an
%   item or property atom, and a value that is an atom, a number, a string
%   or date(Y, M, D) of integers.
wikidata_fact(wd(Q, P, V)) :- atom(Q), atom(P), wikidata_value(V).
wikidata_fact(wd_label(E, L)) :- atom(E), string(L).
wikidata_fact(wd_constraint(P, K)) :- atom(P), atom(K).

wikidata_value(V) :- atom(V), !.
wikidata_value(V) :- number(V), !.
wikidata_value(V) :- string(V), !.
wikidata_value(date(Y, M, D)) :- integer(Y), integer(M), integer(D).

unload_wikidata :-
    retractall(wd(_,_,_)), retractall(wd_label(_,_)),
    retractall(wd_constraint(_,_)), retractall(wd_import(_,_)).

%   A user predicate mapped to a Wikidata property: P(E, Y) holds per
%   Wikidata when E is the same as item Q and Q's property has value Y
%   (for a date, its year).
wd_claim(G) :-
    G =.. [Pred, E, Y],
    fact_ref(_, wd_property(Pred, P), _),
    fact_ref(_, same_as(E, Q), _),
    wd(Q, P, V),
    wd_value(V, Y).

wd_value(date(Y, _, _), Y) :- !.
wd_value(V, V).

		 /*******************************
		 *         SUGGESTIONS          *
		 *******************************/

%!  suggestion(-S) is nondet.
%   Suggestions without a model (docs/translator.md § Suggesting facts,
%   rules and constraints): S is suggestion(Kind, Facts, Evidence, Score),
%   Facts what adopting it adds, Evidence a string.

:- discontiguous suggestion/1.

suggestion(suggestion(link, [same_as(E, Q)], Ev, Score)) :-
    entity_name(E, Name),
    \+ fact_ref(_, same_as(E, _), _),
    split_string(Name, " _'", " _'", Words0),
    include([W0]>>(string_length(W0, N0), N0 >= 3), Words0, Words),
    string_lower(Name, LName),
    wd_label(Q, L),
    string_lower(L, LL),
    (   LL == LName -> Match = 5                         % the whole name
    ;   member(W, Words), string_lower(W, LL) -> Match = 1   % one word of it
    ),
    aggregate_all(count, (wd(Q, _, date(_, _, _))), Dated),
    (   fact_ref(_, F, _), F =.. [_, E, Y], integer(Y), wd(Q, _, date(Y2, _, _)), abs(Y - Y2) =< 5
    ->  Near = 10 ; Near = 0
    ),
    Score is Match + Dated + Near,
    (   Near > 0 -> NearText = "; Wikidata dates it within five years of yours" ; NearText = "" ),
    format(string(Ev), "the label “~w” of ~w names ~w~w", [L, Q, Name, NearText]).

suggestion(suggestion(mapping, [wd_property(Pred, P)], Ev, 5)) :-
    fact_ref(_, same_as(E, Q), _),
    fact_ref(_, F, _), F =.. [Pred, E, Y], integer(Y),
    \+ memberchk(Pred, [same_as, wd_property, label, node_def, rule_def, claims]),
    \+ fact_ref(_, wd_property(Pred, _), _),
    wd(Q, P, date(Y2, _, _)),
    property_name(P, PN),
    format(string(Ev), "~w’s ~w (~w) is ~w; your ~w gives ~w", [Q, PN, P, Y2, Pred, Y]).

suggestion(suggestion(constraint(Name, [E, A, B], Body), [], Ev, 4)) :-
    fact_ref(_, wd_property(Pred, P), _),
    wd_constraint(P, single_value),
    G1 =.. [Pred, E, A], G2 =.. [Pred, E, B],
    Body = (G1, G2, A \= B),
    \+ ( fact_ref(_, node_def(_, constraint(_, _, B0)), _), B0 =@= Body ),
    property_name(P, PN),
    format(string(Name), "one ~w per entity (Wikidata ~w)", [Pred, P]),
    format(string(Ev), "Wikidata gives ~w (~w) a single-value constraint", [P, PN]).

suggestion(suggestion(rule(Head, Body), [], Ev, Score)) :-
    user_binary(P), user_binary(Q), P \== Q,
    B =.. [P, X, Y], H =.. [Q, X, Y],
    aggregate_all(count, (beliefs(B)), NB), NB >= 2,
    aggregate_all(count, (beliefs(B), beliefs(H)), Support), Support >= 2,
    Conf is Support / NB, Conf >= 0.8,
    \+ ( fact_ref(_, rule_def(_, H0, B0), _), H0-B0 =@= H-B ),
    Exceptions is NB - Support,
    Head = H, Body = B,
    Score is Support,
    plural(Exceptions, Pl),
    format(string(Ev), "support ~w, confidence ~2f, ~w exception~w", [Support, Conf, Exceptions, Pl]).

plural(1, "") :- !.
plural(_, "s").

%   A reference for the claims about entity E: a work in the store whose
%   main subject is E's item, or that works in the store cite. The memex
%   turns E into the items that state facts about it.
suggestion(suggestion(reference(E, Work), [], Ev, Score)) :-
    fact_ref(_, same_as(E, Q), _),
    \+ wd(Q, 'P31', 'Q13442814'),
    in_store_work(Work, QW),
    (   wd(QW, 'P921', Q) -> S1 = 3, Why = "its main subject is the same item" ; S1 = 0, Why = "" ),
    aggregate_all(count, (in_store_work(_, Q2), wd(Q2, 'P2860', QW)), Cited),
    Score is S1 + Cited,
    S1 > 0,
    plural(Cited, Pl),
    format(string(Ev), "~w (~w); cited by ~w work~w in the store", [Why, QW, Cited, Pl]).

%   A work in the store: an entity the user linked to an item of Wikidata
%   that is a scholarly article. Only these are ever suggested as references.
in_store_work(W, QW) :-
    fact_ref(_, same_as(W, QW), _),
    wd(QW, 'P31', 'Q13442814').

%   Each user predicate of two arguments, once.
user_binary(P) :-
    setof(P0, user_binary_(P0), Ps),
    member(P, Ps).
user_binary_(P) :-
    fact_ref(_, F, _), compound(F), F =.. [P, _, _],
    \+ memberchk(P, [same_as, wd_property, label, node_def, rule_def, claims, cites]).

entity_name(E, Name) :-
    fact_ref(_, F, _), compound(F), F =.. [P, E | _], atom(E),
    \+ memberchk(P, [same_as, wd_property, label, node_def, rule_def, claims]),
    (   fact_ref(_, label(E, L), _) -> Name = L ; atom_string(E, Name) ).

property_name(P, Name) :- ( wd_label(P, L) -> Name = L ; Name = P ).

%!  suggestions(-L) is det.
%   Every suggestion, each once, best evidence first.
suggestions(L) :-
    findall(S, suggestion(S), L0),
    variants_once(L0, L1),
    sort(4, @>=, L1, L).

%   Keep one of each set of suggestions equal up to renaming variables.
variants_once(L0, L) :-
    findall(K-S, (member(S, L0), copy_term(S, K), numbervars(K, 0, _)), KS),
    sort(1, @<, KS, Unique),
    pairs_values(Unique, L).
