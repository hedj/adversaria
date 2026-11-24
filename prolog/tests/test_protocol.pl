:- encoding(utf8).
% Tests of prolog/protocol.pl. Each test says what fault it catches.
% Run: swipl -g run_tests -t halt prolog/tests/test_protocol.pl

:- use_module('../protocol').
:- use_module(gen_terms).

:- use_module(library(plunit)).
:- use_module(library(time)).
:- use_module(library(memfile)).
:- use_module(library(random)).

round_trip(T) :-
    terms_cells([T], Cells, _, _),
    cells_terms(Cells, [T2], _),
    (   T2 =@= T -> true ; format(user_error, "~q came back as ~q~n", [T, T2]), fail ).

:- begin_tests(protocol).

test(ten_thousand_round_trips) :-
    % Catches any term that does not come back as itself.
    set_random(seed(2)),
    forall(between(1, 10000, _), (gen_term(0, T), round_trip(T))).

test(big_integers) :-
    % Catches integers beyond 64 bits truncated (the generator stays within 64).
    forall(member(E, [2^100, -(2^200), 2^63, -(2^63)-1]), (I is E, integer(I), round_trip(I))).

test(generator_makes_integers) :-
    % Catches the generator yielding unevaluated expressions instead of integers.
    set_random(seed(3)),
    forall(between(1, 200, _), (gen_terms:gen(1, 0, T), assertion(integer(T)))).

test(shared_variables) :-
    % Catches two occurrences of one variable decoded as two variables.
    terms_cells([f(X, Y, X), g(Y)], Cells, Vars, _),
    assertion(Vars == [X, Y]),
    cells_terms(Cells, [f(A, B, C), g(D)], Vs),
    assertion((A == C, B == D, A \== B, Vs == [A, B])).

test(int_boundaries) :-
    % Catches the 29-bit INT range off by one: the edges stay INT, beyond is BLOB.
    forall(member(I-Tag, [268435455-1, -268435456-1, 268435456-4, -268435457-4]),
           ( terms_cells([I], [C|_], _, _), T is C >> 29, assertion(T == Tag) )).

test(negative_zero) :-
    % Catches -0.0 coming back as 0.0.
    terms_cells([-0.0], Cells, _, _),
    cells_terms(Cells, [Z], _),
    assertion(Z == -0.0),
    assertion(\+ Z == 0.0).

test(lists_are_dot) :-
    % Catches a list cell sent under SWI-Prolog's own name '[|]'.
    terms_cells([[a]], [C|_], _, _),
    No is C /\ ((1 << 21) - 1), Ar is (C >> 21) /\ 0xff,
    assertion(No-Ar == 1-2).

refused_term('[]').
refused_term(T) :- compound_name_arguments(T, '.', [a, b]).  % '.'(a,b) in source means a dict call
refused_term(T) :- compound_name_arguments(T, foo, []).
refused_term(T) :- T is inf.
refused_term(T) :- T is nan.
refused_term(T) :- T is 1r3.  % a rational: atomic, but no number of the protocol

test(refused, [forall(refused_term(T)), throws(protocol_error(_))]) :-
    % Catches terms that would clash with lists, or have no cell form.
    terms_cells([T], _, _, _).

nested(0, x) :- !.
nested(N, f(T)) :- N1 is N - 1, nested(N1, T).

test(nesting_limited) :-
    % Catches a term deeper than the protocol allows sent anyway, which the
    % client would refuse, and a limit that counts a list's spine.
    nested(256, T256), terms_cells([T256], _, _, _),
    nested(257, T257),
    catch(terms_cells([T257], _, _, _), E, true),
    assertion(E == protocol_error(nested_deeper_than(256))),
    numlist(1, 100000, L), terms_cells([L], _, _, _),
    findall([I], between(1, 1000, I), LL), terms_cells([LL], _, _, _).

test(new_atoms_reported) :-
    % Catches an atom used before the client is told its name.
    terms_cells([zz_new_atom(zz_other)], _, _, New),
    assertion(New == [zz_new_atom, zz_other]),
    terms_cells([zz_new_atom], _, _, New2),
    assertion(New2 == []).

test(message_round_trip) :-
    % Catches a framing error: header fields, byte order or length.
    terms_cells([f("é", 1.5, [x])], Body, _, _),
    new_memory_file(M),
    open_memory_file(M, write, Out, [encoding(octet)]),
    write_message(Out, answer, 4000000000, Body),
    close(Out),
    memory_file_to_codes(M, Bytes, octet),
    Bytes = [B0, B1, _, _ | _],
    assertion(B0 == 7), assertion(B1 == 0),
    open_memory_file(M, read, In, [encoding(octet)]),
    read_message(In, Type, Req, Body2),
    close(In),
    assertion(Type-Req == answer-4000000000),
    assertion(Body2 == Body).

test(strings_of_ascii_and_of_other_letters_round_trip) :-
    % Catches a string read byte for letter where its bytes are not all
    % ASCII, or an ASCII one changed on its way.
    Strings = ["plain ASCII, with \x7f\ and a NUL \x0\", "é", "東京", "a\U0001F600\b"],
    terms_cells(Strings, Cells, _, _),
    cells_terms(Cells, Back, []),
    assertion(Back == Strings).

test(a_string_not_utf8_is_refused, [forall(member(Bytes, [[0xff], [0x41, 0xc3]])), throws(protocol_error(bad_utf8))]) :-
    % Catches bytes that are not UTF-8 taken as letters. (An overlong
    % form, such as C0 80, library(utf8) still accepts.)
    blob_cells([0's|Bytes], Cells),
    cells_terms(Cells, _, _).

%   A string blob's cells for these bytes, its kind first.
blob_cells(Bytes, [Head|Words]) :-
    length(Bytes, N),
    Head is (4 << 29) \/ N,
    Pad is (4 - N mod 4) mod 4,
    length(Zeros, Pad), maplist(=(0), Zeros),
    append(Bytes, Zeros, Padded),
    bytes_words(Padded, Words).

bytes_words([], []).
bytes_words([A,B,C,D|T], [W|Ws]) :-
    W is (A << 24) \/ (B << 16) \/ (C << 8) \/ D,
    bytes_words(T, Ws).

test(malformed, [forall(member(Cells, [[], [0x60000001], [0x80000005, 0x73000000]])), throws(protocol_error(_))]) :-
    % Catches a truncated term, an unknown tag, or a short blob accepted.
    cells_terms(Cells, [_|_], _).

test(variable_numbers_must_be_dense, [throws(protocol_error(variable_numbers_not_dense(1, 0)))]) :-
    % Catches VAR 1 accepted without VAR 0, which would misalign the answers.
    cells_terms([1], _, _).

test(a_cyclic_term_is_refused, [throws(protocol_error(cyclic_term))]) :-
    % Catches the encoder looping for ever on a cyclic list, whose tail is
    % at the list's own depth, so the depth limit never stops it: `query
    % X = [a|X]` hung the server (the code review, C-B7).
    X = [a|X],
    call_with_time_limit(5, terms_cells([X], _, _, _)).

test(many_variables_are_numbered_in_linear_time) :-
    % Catches variable numbering that searches the variables met: 20,000
    % took about 28 s, 30,000 took 65 s, past the client's 60 s wait (the
    % code review, C-B8). The variables come back numbered in order, and
    % none keeps the encoder's attribute.
    length(L, 20000),
    call_with_time_limit(10, terms_cells([L, L], Cells, Vars, _)),
    Vars == L,
    cells_terms(Cells, [L2, L3], _), L2 == L3, length(L2, 20000),
    \+ ( member(V, L), attvar(V) ).

:- end_tests(protocol).
