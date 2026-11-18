:- encoding(utf8).
/** <module> Reading controlled English as the notebook's statements

ace_read/2 reads a text of ACE, through APE's logical form (its DRS),
as the statements the memex keeps: facts, claims, rules and constraints
(docs/design.md, § Reading controlled English). A text it cannot read
whole is not read at all: every condition of the DRS must be used.

A discourse referent stands for its value. A noun phrase is a goal on
it: "the date of X" is date(X, R), and "the income", with no owner, is
the box cell(income, R). "is" makes two values one: R = 1988, or, for a
sum, R is A + B. So a fact, a rule's body and a constraint's body come
from one mapping:

| ACE | Read as |
| The date of Buran-flight is 1988. | fact(date(buran_flight, 1988)) |
| The income is 2000. | fact(cell(income, 2000)) |
| Buran is a:reusable. | fact(reusable(buran)) |
| Buran-flight follows Energia-launch. | fact(follow(buran_flight, energia_launch)) |
| Talk-notes says that S. | claim("Talk-notes", F) for each fact F of S |
| If C then the rest is X - Y. | rule(rest(R), (C, R is X - Y)) |
| If C then X + Y =< Z. | constraint((C, \+ X + Y =< Z)) |
| If C then it is false that S. | constraint((C, S)) |
| It is false that S. | constraint(S) |
| Nothing has more than 1 date. | constraint((date(E, A), date(E, B), A \== B)) |

A rule's value has a name of its own: "If C then the rest is X - Y" is a
rule for rest(R), not for the box cell(rest, R), as a rule may not read
what it defines. So "the rest" is rest(R) wherever a rule defines rest,
in the world (the names the memex gives) or in the same text.

A proper name is an entity: its letters in lower case, its hyphens made
underscores (Buran-flight is buran_flight), as the notebook's names are;
a name the notebook gives (ace.rs, lexicon_of) comes back as it is.
*/

:- module(ace_read, [ace_read/2, ace_read/3, ace_read_text/3]).

:- use_module(ace, [ace_drs/4]).
:- use_module(library(apply)).
:- use_module(library(lists)).

%!  ace_read(+Text:string, -Result) is det.
%   Result is read(Paraphrase, Statements, Messages) when Text is ACE
%   and reads whole as statements; unread(Paraphrase, Messages) when it
%   is ACE but some of it is not something the notebook states; and
%   not_ace(Messages) when it is not ACE. Statements are fact(F),
%   claim(Source, F), rule(Head, Body) and constraint(Body), their
%   variables numbered ('$VAR'(0), ...): the query protocol sends no
%   variable a query did not have.
ace_read(Text, Result) :-
    ace_read(Text, [], Result).

%!  ace_read(+Text:string, +Rules:list(atom), -Result) is det.
%   As ace_read/2, where Rules are the names of the world's rules of one
%   argument: "the N", for N one of them, is N(R), not cell(N, R).
ace_read(Text, Rules, Result) :-
    ace_drs(Text, Paraphrase, Drs, Messages),
    (   var(Paraphrase)
    ->  Result = not_ace(Messages)
    ;   plain(Drs, drs(_, Conditions)),
        statements(Conditions, Read)
    ->  ruled(Rules, Read, Statements),
        numbervars(Statements, 0, _),
        Result = read(Paraphrase, Statements, Messages)
    ;   Result = unread(Paraphrase, Messages)
    ).

%!  ace_read_text(+Text:string, +Rules:list(atom), -Result) is det.
%   As ace_read/3, with each statement as the text of the command the
%   memex runs for it, its terms written as Prolog writes them (quoted,
%   with the brackets their operators need): said(Kind, Text), Kind fact,
%   claim(Source), rule ("Head :- Body") or constraint ("false :- Body").
ace_read_text(Text, Rules, Result) :-
    ace_read(Text, Rules, Read),
    (   Read = read(P, Statements, Ms)
    ->  maplist(statement_text, Statements, Texts),
        Result = read(P, Texts, Ms)
    ;   Result = Read
    ).

statement_text(fact(F), said(fact, T)) :- written(F, T).
statement_text(claim(Source, F), said(claim(Source), T)) :- written(F, T).
statement_text(rule(H, B), said(rule, T)) :- written((H :- B), T).
statement_text(constraint(B), said(constraint, T)) :- written((false :- B), T).

written(Term, Text) :-
    format(string(Text), "~W", [Term, [quoted(true), numbervars(true), spacing(next_argument)]]).

%   The statements with each box a rule defines read as the rule's
%   value: the rules of the world, and those the statements make.
ruled(Rules, Read, Statements) :-
    convlist(rule_name, Read, Made),
    append(Rules, Made, Names),
    maplist(ruled_statement(Names), Read, Statements).

ruled_statement(Names, S, RS) :- ruled_kind(S, Names, RS).

ruled_kind(rule(H, B), Names, rule(RH, RB)) :- ruled_head(H, RH), ruled_goals(B, Names, RB).
ruled_kind(constraint(B), Names, constraint(RB)) :- ruled_goals(B, Names, RB).
ruled_kind(fact(F), Names, fact(RF)) :- ruled_goals(F, Names, RF).
ruled_kind(claim(S, F), Names, claim(S, RF)) :- ruled_goals(F, Names, RF).

rule_name(rule(cell(N, _), _), N).

ruled_head(cell(N, R), H) :- !, H =.. [N, R].
ruled_head(H, H).

%   A goal, or a conjunction of goals, with each box a rule defines read
%   as the rule's value.
ruled_goals(G, Names, RG) :-
    (   var(G)
    ->  RG = G
    ;   G = (A, B)
    ->  RG = (RA, RB), ruled_goals(A, Names, RA), ruled_goals(B, Names, RB)
    ;   G = (\+ A)
    ->  RG = (\+ RA), ruled_goals(A, Names, RA)
    ;   G = cell(N, R), atom(N), memberchk(N, Names)
    ->  RG =.. [N, R]
    ;   RG = G
    ).

%   A DRS without APE's places (Condition-Sentence/Token) and with its
%   groups of conditions made one list.
plain(drs(Refs, Conds), drs(Refs, Plain)) :-
    flatten(Conds, Flat),
    maplist(plain_condition, Flat, Plain).

plain_condition(C-_, P) :- !, plain_condition(C, P).
plain_condition(A => B, PA => PB) :- !, plain(A, PA), plain(B, PB).
plain_condition(-D, -P) :- !, plain(D, P).
plain_condition(L:D, L:P) :- !, plain(D, P).
plain_condition(C, C).

%   The statements a DRS's top level makes: its implications and
%   negations each a rule or a constraint, and the rest facts and claims.
statements(Conditions, Statements) :-
    partition(complex, Conditions, Complex, Simple),
    maplist(complex_statement, Complex, Made),
    simple_statements(Simple, Stated),
    append(Made, Stated, Statements),
    Statements \== [].

complex(_ => _).
complex(-_).

complex_statement(drs(_, If) => drs(_, Then), Statement) :-
    goals(If, Body),
    then_statement(Then, Body, Statement).
complex_statement(-drs(_, Conds), constraint(Body)) :-
    goals(Conds, Goals),
    conjunction(Goals, Body).

%   What follows from If's goals: a value (a rule), a comparison that
%   must hold, or a state that must not.
then_statement([-drs(_, Inner)], [], constraint((G1, G2, V1 \== V2))) :-
    more_than_one(Inner, N, A),
    !,
    G1 =.. [N, A, V1],
    G2 =.. [N, A, V2].
then_statement([-drs(_, Conds)], If, constraint(Body)) :-
    !,
    goals(Conds, Not),
    append(If, Not, Goals),
    conjunction(Goals, Body).
then_statement([formula(L, Op, R)], If, constraint(Body)) :-
    !,
    comparison(Op, L, R, Must),
    append(If, [\+ Must], Goals),
    conjunction(Goals, Body).
then_statement(Then, If, rule(Head, Body)) :-
    goals(Then, [Head|Rest]),
    \+ comparison_goal(Head),
    append(If, Rest, Goals),
    conjunction(Goals, Body).

%   "has more than 1 N": a count of N greater than 1 that A has, in
%   either order.
more_than_one(Inner, N, A) :-
    length(Inner, 2),
    member(object(B, N, countable, na, greater, 1), Inner),
    member(predicate(_, have, A, B0), Inner),
    B0 == B.

%   The facts and claims of conditions with no implication or negation:
%   their goals, each ground, and each claim a sub-DRS's facts.
simple_statements(Conds, Statements) :-
    partition(claim_condition(Conds), Conds, Claims, Rest),
    exclude(claimed_drs(Claims), Rest, Own),
    goals(Own, Goals),
    maplist(fact_statement, Goals, Facts),
    maplist(claim_statements(Conds), Claims, Said),
    append([Facts|Said], Statements).

fact_statement(G, fact(G)) :- ground(G).

%   A condition that says a source states something: say, claim, state,
%   believe or report, with a sub-DRS as its object.
claim_condition(Conds, predicate(_, Verb, named(_), Label)) :-
    memberchk(Verb, [say, claim, state, believe, report]),
    member(L:_, Conds), L == Label, !.

claimed_drs(Claims, L:_) :-
    member(predicate(_, _, _, Label), Claims), Label == L, !.

claim_statements(Conds, predicate(_, _, named(Source), Label), Claims) :-
    member(L:drs(_, Said), Conds), L == Label, !,
    atom_string(Source, Text),
    simple_statements(Said, Facts),
    maplist(claim_of(Text), Facts, Claims).

claim_of(Source, fact(F), claim(Source, F)).

%   The goals of a list of conditions, every condition used. The goals
%   are found while the referents are still variables, so an owner is
%   found by its referent; then each "is" binds its two values.
goals(Conds, Goals) :-
    partition(binding(Conds), Conds, Bindings, Others),
    used_all(Others),
    convlist(goal(Others), Others, Found),
    foldl(bind, Bindings, [], Arithmetic),
    append(Found, Arithmetic, Goals).

%   An "is" between a value and another value or an expression, not a
%   property: "is reusable" and "is greater than" are goals.
binding(Conds, predicate(_, be, _, V)) :-
    \+ ( member(P, Conds), property_of(P, V) ).

%   Bind an "is": the two values made one, or, for an expression, a goal
%   that computes it.
bind(predicate(_, be, R, V), Acc, Acc) :-
    value(V, X), \+ expression(X), !,
    R = X.
bind(predicate(_, be, R, expr(Op, A, B)), Acc, [R is E|Acc]) :-
    expression_value(expr(Op, A, B), E).

expression(X) :- compound(X), X \= '$VAR'(_), functor(X, F, 2), memberchk(F, [+, -, *, /]).

%   A goal of a condition, or none for one that only names a variable
%   (something) or is used by another condition (an owner's "of", an
%   adjective's property).
goal(_, object(_, something, dom, na, na, na), _) :- !, fail.
goal(_, relation(_, of, _), _) :- !, fail.
goal(_, property(_, _, _), _) :- !, fail.
goal(_, property(_, _, _, _), _) :- !, fail.
goal(Conds, object(R, N, countable, na, eq, 1), G) :-
    !,
    (   member(relation(O, of, Owner), Conds), O == R
    ->  value(Owner, X), G =.. [N, X, R]
    ;   G = cell(N, R)
    ).
goal(Conds, predicate(_, be, S, A), G) :-
    !,
    member(P, Conds), property_of(P, A),
    property_goal(P, S, G).
goal(_, predicate(_, Verb, S), G) :-
    !,
    value(S, X), G =.. [Verb, X].
goal(_, predicate(_, Verb, S, O), G) :-
    !,
    value(S, X), value(O, Y), G =.. [Verb, X, Y].
goal(_, formula(L, Op, R), G) :-
    comparison(Op, L, R, G).

property_of(property(A0, _, _), A) :- A0 == A.
property_of(property(A0, _, _, _), A) :- A0 == A.

%   "is Adjective" (pos), or "is greater/less than" a value.
property_goal(property(_, Adj, pos), S, G) :-
    value(S, X), G =.. [Adj, X].
property_goal(property(_, great, comp_than, C), S, X > Y) :- value(S, X), value(C, Y).
property_goal(property(_, large, comp_than, C), S, X > Y) :- value(S, X), value(C, Y).
property_goal(property(_, small, comp_than, C), S, X < Y) :- value(S, X), value(C, Y).
property_goal(property(_, little, comp_than, C), S, X < Y) :- value(S, X), value(C, Y).

%   Every condition is one the mapping knows: one it does not know is a
%   part of the text the notebook cannot state.
used_all(Others) :- forall(member(C, Others), known(C)).

known(object(_, something, dom, na, na, na)).
known(object(_, _, countable, na, eq, 1)).
known(relation(_, of, _)).
known(property(_, _, pos)).
known(property(_, _, comp_than, _)).
known(predicate(_, _, _)).
known(predicate(_, _, _, _)).
known(formula(_, Op, _)) :- comparison(Op, _, _, _).

%   A value: a referent (a variable, or bound by "is"), a number, a
%   string, an entity, or an expression of values.
value(V, V) :- var(V), !.
value(int(N), N) :- !.
value(string(S), T) :- !, atom_string(S, T).
value(named(Name), Entity) :- !, entity(Name, Entity).
value(expr(Op, A, B), E) :- !, expression_value(expr(Op, A, B), E).
value(V, V) :- atomic(V).

expression_value(expr(Op, A, B), E) :-
    memberchk(Op, [+, -, *, /]),
    value(A, X), value(B, Y),
    E =.. [Op, X, Y].

%   A proper name as the notebook's entity.
entity(Name, Entity) :-
    downcase_atom(Name, Lower),
    atomic_list_concat(Parts, '-', Lower),
    atomic_list_concat(Parts, '_', Entity).

comparison(Op, L, R, G) :-
    memberchk(Op-P, [(<)-(<), (>)-(>), (=<)-(=<), (>=)-(>=), (=)-(=:=)]),
    value(L, X), value(R, Y),
    G =.. [P, X, Y].

comparison_goal(G) :- compound(G), functor(G, F, 2), memberchk(F, [<, >, =<, >=, =:=, \==]).

conjunction([G], G) :- !.
conjunction([G|Gs], (G, C)) :- conjunction(Gs, C).
