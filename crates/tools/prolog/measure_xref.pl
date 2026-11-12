/*  For crates/tools' interactivity measure: for each predicate defined in
    the given Prolog files, the predicates of those files it calls, found by
    SWI-Prolog's own cross-referencer (library(prolog_xref)). Prints one
    line per predicate: File, Name/Arity and its callees, tab-separated.

    swipl -q -g main -t halt measure_xref.pl -- FILE...
*/
:- use_module(library(prolog_xref)).

main :-
    current_prolog_flag(argv, Files),
    maplist([F]>>xref_source(F, [silent(true)]), Files),
    findall(F-PI, (member(F, Files), defined(F, PI)), Defs),
    pairs_values(Defs, PIs0),
    sort(PIs0, PIs),
    forall(member(F-N/A, Defs),
           ( functor(H, N, A),
             findall(C, ( xref_called(F, Callee, H), callee_pi(Callee, C), memberchk(C, PIs), C \== N/A ), Cs0),
             sort(Cs0, Cs),
             format("~w\t~w/~w\t", [F, N, A]),
             forall(member(CN/CA, Cs), format("~w/~w ", [CN, CA])),
             nl )).

defined(F, N/A) :-
    xref_defined(F, H, local(_)),
    functor(H, N, A).

callee_pi(_:C, PI) :- !, callee_pi(C, PI).
callee_pi(C, N/A) :- callable(C), functor(C, N, A).
