:- encoding(utf8).
% Tests of prolog/server.pl, served over memory files as the memex talks
% to it. Each test says what fault it catches.
% Run: swipl -g run_tests -t halt prolog/tests/test_server.pl

:- consult('../server').
:- use_module(library(plunit)).
:- use_module(library(memfile)).

:- dynamic known/1.              % the atom numbers the client knows

%   A fresh server's replies, Type-Req-Cells, to messages, Type-Req-Terms.
%   The client knows the atoms it sends (it asked for them with ATOM) and
%   those announced to it.
served(Msgs, Replies) :-
    fresh_server,
    retractall(known(_)), assertz(known(0)), assertz(known(1)),
    new_memory_file(Sent),
    setup_call_cleanup(open_memory_file(Sent, write, W, [encoding(octet)]),
                       forall(member(M, Msgs), send(W, M)), close(W)),
    new_memory_file(Got),
    setup_call_cleanup(open_memory_file(Sent, read, In, [encoding(octet)]),
                       setup_call_cleanup(open_memory_file(Got, write, Out, [encoding(octet)]),
                                          served_once(In, Out), close(Out)),
                       close(In)),
    setup_call_cleanup(open_memory_file(Got, read, R, [encoding(octet)]),
                       replies(R, Replies), close(R)).

%   Serve, and fail if serving left a choice point: one per message grows
%   the local stack without limit.
served_once(In, Out) :-
    call_cleanup(serve(In, Out), Det = true),
    (   Det == true -> true ; format(user_error, "serve/2 left a choice point~n", []), fail ).

%   The server as it starts: no atoms numbered, no queries, no facts.
fresh_server :-
    reset_atoms,
    forall(retract(query_engine(_, E)), engine_destroy(E)),
    forall(retract(fact_ref(_, _, Ref)), erase(Ref)).

send(W, Type-Req-Terms) :-
    terms_cells(Terms, Cells, _, _),
    atoms_in(Cells, Nos), forall(member(No, Nos), assertz(known(No))),
    write_message(W, Type, Req, Cells).

replies(In, Rs) :-
    (   read_message(In, T, Req, Cells) -> Rs = [T-Req-Cells|Rs1], replies(In, Rs1) ; Rs = [] ).

%   The atom numbers in cells: atoms and functors, not inside blobs.
atoms_in([], []).
atoms_in([C|Cs], Nos) :-
    Tag is C >> 29, V is C /\ 0x1fffffff,
    (   Tag =:= 4 -> W is (V + 3) // 4, length(Ws, W), append(Ws, Rest, Cs), atoms_in(Rest, Nos)
    ;   Tag =:= 2 -> Nos = [V|Nos1], atoms_in(Cs, Nos1)
    ;   Tag =:= 3 -> No is V /\ 0x1fffff, Nos = [No|Nos1], atoms_in(Cs, Nos1)
    ;   atoms_in(Cs, Nos)
    ).

%   Every atom a reply uses was announced before it, or sent.
announced([]).
announced([atom_is-_-[C|_]|Rs]) :- !, No is C /\ 0x1fffffff, assertz(known(No)), announced(Rs).
announced([T-Req-Cells|Rs]) :-
    atoms_in(Cells, Nos),
    forall(member(No, Nos),
           (   known(No) -> true
           ;   format(user_error, "~w ~w uses atom ~w, never announced~n", [T, Req, No]), fail
           )),
    announced(Rs).

:- begin_tests(server).

test(serving_leaves_no_choice_point) :-
    % Catches handle/4's last clause, whose variable first argument left a
    % choice point on every message: the server's local stack grew 195 MB
    % over 400,000 messages (the code review, C-B5).
    served([query-1-[true], next-1-[1], cancel-2-[], atom-3-["x"]], _).

test(an_encoding_that_fails_leaves_no_atom_unannounced) :-
    % Catches atoms numbered by an encoding that then failed: the answer
    % 1r3 cannot be encoded, nor can the error that holds it, but
    % protocol_error was numbered on the way, so the next error used it
    % unannounced and the client could never decode it (the code review,
    % C-B1).
    served([query-1-[_ is 1 rdiv 3], next-1-[1], next-9-[1]], Rs),
    announced(Rs).

test(an_answer_too_long_to_send_finishes_its_query) :-
    % Catches the engine of a query whose answer is too long for one
    % message left alive for good: the send was outside the catch that
    % finishes the query (the code review, C-B4).
    served([query-1-[findall(a, between(1, 70000, _), _)], next-1-[1]], Rs),
    last(Rs, done-1-[_|Error]),
    cells_terms(Error, [protocol_error(message_too_long(_))], _),
    \+ query_engine(1, _).

:- end_tests(server).

%   server.pl's own main serves standard input; this file's runs the tests.
:- initialization((run_tests -> halt(0) ; halt(1)), main).
