:- encoding(utf8).
/** <module> The memex's Prolog side: the query protocol over standard I/O

Run as a child process of the memex: swipl prolog/server.pl
It reads messages on standard input and writes them on standard output
(docs/design.md § Query protocol). Each query runs in its own engine, so it
computes an answer only when NEXT asks for one.

| Message        | Body                          | Reply                              |
| QUERY          | the goal                      | none; DONE 1 if it cannot start    |
| NEXT           | n                             | up to n ANSWERs; DONE 0 when none are left |
| CANCEL         | nothing                       | DONE 2                             |
| ASSERT/RETRACT | the fact, its journal seq     | DONE 0, or DONE 1 and the error    |
| ATOM           | a name, as a string BLOB      | ATOM_IS: the number and the name   |

DONE carries a status: 0 no more answers (or done), 1 error and the error
term, 2 cancelled. A garbled header kills the server (exit status 1). Before any message that uses an atom new to the client,
the server sends ATOM_IS for it, with the same request number.
*/

:- use_module(protocol).
:- use_module(library(lists)).

:- dynamic query_engine/2.       % Req, Engine
:- dynamic fact_ref/3.           % Seq, Fact, ClauseRef

:- initialization(main, main).

main :-
    current_output(Out),
    set_stream(Out, type(binary)),
    set_stream(user_input, type(binary)),
    % Goals that print must not write into the protocol: send them to stderr.
    set_stream(user_error, alias(user_output)),
    set_output(user_error),
    nb_setval(protocol_out, Out),
    serve(user_input, Out).

%   A clean end of input ends the server with status 0. A garbled or cut
%   header means the stream is out of step and no later message can be
%   trusted, so the server dies: it says why and exits with status 1. A
%   well-framed request with a bad body only gets an error reply.
serve(In, Out) :-
    (   catch(read_message(In, Type, Req, Body), E, die(E))
    ->  catch(handle(Type, Req, Body, Out), E2, reply_error(Out, Req, E2)),
        serve(In, Out)
    ;   true
    ).

die(E) :-
    format(user_error, "prolog/server.pl: dying on a garbled message: ~q~n", [E]),
    halt(1).

handle(query, Req, Body, _) :-
    cells_terms(Body, Terms, Vars),
    (   Terms = [Goal] -> true ; throw(protocol_error(query_needs_one_goal)) ),
    (   query_engine(Req, _) -> throw(protocol_error(request_in_use(Req))) ; true ),
    engine_create(Vars, user:Goal, E),
    assertz(query_engine(Req, E)).
handle(next, Req, Body, Out) :-
    cells_terms(Body, [N], []),
    (   integer(N), N >= 0 -> true ; throw(protocol_error(next_needs_count)) ),
    (   query_engine(Req, E) -> true ; throw(protocol_error(unknown_request(Req))) ),
    next_answers(N, Req, E, Out).
handle(cancel, Req, _, Out) :-
    (   retract(query_engine(Req, E)) -> engine_destroy(E) ; throw(protocol_error(unknown_request(Req))) ),
    done(Out, Req, 2).
handle(assert, Req, Body, Out) :-
    cells_terms(Body, [Fact, Seq], _),
    fact_seq(Fact, Seq),
    declare(Fact),
    assertz(user:Fact, Ref),
    assertz(fact_ref(Seq, Fact, Ref)),
    done(Out, Req, 0).
handle(retract, Req, Body, Out) :-
    cells_terms(Body, [Fact, Seq], _),
    fact_seq(Fact, Seq),
    (   fact_ref(Seq, F, Ref), F =@= Fact
    ->  erase(Ref), retract(fact_ref(Seq, F, Ref))
    ;   throw(protocol_error(no_such_fact(Seq)))
    ),
    done(Out, Req, 0).
handle(atom, Req, Body, Out) :-
    cells_terms(Body, [Name0], []),
    (   string(Name0) -> atom_string(Name, Name0) ; throw(protocol_error(atom_needs_string)) ),
    intern(Name, No, _),
    atom_is(Out, Req, Name, No).
handle(Type, _, _, _) :-
    throw(protocol_error(not_a_request(Type))).

%!  memex_fact(?Seq, ?Fact) is nondet.
%   The facts the memex added, in the order it added them.
memex_fact(Seq, Fact) :- fact_ref(Seq, Fact, _).

fact_seq(Fact, Seq) :-
    (   callable(Fact) -> true ; throw(protocol_error(fact_not_callable)) ),
    (   integer(Seq), Seq >= 1 -> true ; throw(protocol_error(seq_not_positive)) ).

%   A fact's predicate is dynamic and incremental, so tabled queries over it
%   are recomputed when it changes (docs/design.md § Dataflow).
declare(Fact) :-
    functor(Fact, N, A),
    (   current_predicate(user:N/A) -> true ; dynamic([user:N/A], [incremental(true)]) ).

next_answers(0, _, _, _) :- !.
next_answers(N, Req, E, Out) :-
    (   catch(engine_next(E, Vals), Err, (finish(Req, E), throw(Err)))
    ->  catch(terms_cells(Vals, Cells, _, New), EncErr, (finish(Req, E), throw(EncErr))),
        announce(Out, Req, New),
        send(Out, answer, Req, Cells),
        N1 is N - 1,
        next_answers(N1, Req, E, Out)
    ;   finish(Req, E),
        done(Out, Req, 0)
    ).

finish(Req, E) :-
    retractall(query_engine(Req, _)),
    engine_destroy(E).

announce(Out, Req, Names) :-
    forall(member(Name, Names), ( intern(Name, No, _), atom_is(Out, Req, Name, No) )).

atom_is(Out, Req, Name, No) :-
    atom_string(Name, S),
    terms_cells([S], [Blob|Rest], _, _),
    Cell is (2 << 29) \/ No,
    send(Out, atom_is, Req, [Cell, Blob|Rest]).

send(Out, Type, Req, Body) :- write_message(Out, Type, Req, Body).

%   An error goes back as DONE 1 with the error term. A term that cannot be
%   encoded (it holds a stream, say) goes back as its printed text.
reply_error(Out, Req, E) :-
    (   catch(terms_cells([E], Cells, _, New), _, fail)
    ->  true
    ;   format(string(S), "~q", [E]),
        terms_cells([S], Cells, _, New)
    ),
    announce(Out, Req, New),
    status_cell(1, Status),
    send(Out, done, Req, [Status|Cells]).

%   DONE's status is an INT cell: 0 done, 1 error, 2 cancelled.
done(Out, Req, Status) :-
    status_cell(Status, C),
    send(Out, done, Req, [C]).

status_cell(K, C) :- C is (1 << 29) \/ K.
