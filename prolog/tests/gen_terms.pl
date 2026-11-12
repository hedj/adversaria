:- module(gen_terms, [gen_term/2, seeded_term/2, same_as_seeded/2]).
:- encoding(utf8).
% Random terms for the protocol's tests (prolog/tests/test_protocol.pl, and
% the cross-language test in crates/acceptance/tests/milestone2.rs).

:- use_module(library(random)).
:- use_module(library(yall)).
:- use_module(library(apply)).
:- use_module(library(lists)).

%!  gen_term(+Depth, -T) is det.
%   A random term: atoms, integers (small and large), floats, strings,
%   variables, lists and nested compounds.

gen_term(D, T) :-
    (   D >= 4 -> random_between(0, 6, K) ; random_between(0, 8, K) ),
    gen(K, D, T).

gen(0, _, T) :- random_member(T, [a, foo, 'Bar baz', 'é€😀', [], '[|]', '', ' ']).
gen(1, _, T) :- random_member(E, [0, 1, -1, 268435455, -268435456, 268435456, -268435457,
                                  9007199254740993, 2^63-1, -(2^63)]),
                T is E.   % evaluated: the list holds expressions
gen(2, _, T) :- random_between(-1000000, 1000000, I), T = I.
gen(3, _, T) :- random(R), random_between(-300, 300, E), random_member(S, [1, -1]),
                T0 is S * R * 10.0**E, random_member(T, [T0, 0.0, -0.0, 1.0, 0.1, 1.0e23, 5.0e-324]).
gen(4, _, T) :- random_between(0, 6, N), length(Cs, N),
                maplist([C]>>random_between(1, 0x2fff, C), Cs), string_codes(T, Cs).
gen(5, _, _).
gen(6, _, T) :- random_member(T, ["", "a\nb", "😀"]).
gen(7, D, T) :- D1 is D + 1, random_between(1, 4, N), length(Args, N),
                maplist(gen_term(D1), Args),
                random_member(F, [f, g, supports, '€', '.x']),
                compound_name_arguments(T, F, Args).
gen(8, D, T) :- D1 is D + 1, random_between(0, 3, N), length(T, N), maplist(gen_term(D1), T).

%!  seeded_term(+Seed, -T) is det.
%   The term generated from Seed; the same Seed gives the same term.
seeded_term(Seed, T) :- set_random(seed(Seed)), gen_term(0, T).

%!  same_as_seeded(+Seed, +T) is semidet.
%   T is a variant of the term generated from Seed.
same_as_seeded(Seed, T) :- seeded_term(Seed, T0), T0 =@= T.
