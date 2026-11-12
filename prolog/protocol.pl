:- module(protocol,
          [ read_message/4,          % +In, -Type, -Req, -Body
            write_message/4,         % +Out, +Type, +Req, +Body
            terms_cells/4,           % +Terms, -Cells, -Vars, -NewAtoms
            cells_terms/3,           % +Cells, -Terms, -Vars
            intern/3,                % +Name, -Number, -New
            atom_name/2,             % ?Atom, ?Name  (Prolog atom <-> protocol name)
            message_type/2,          % ?Name, ?Code
            reset_atoms/0
          ]).
:- encoding(utf8).
:- use_module(library(utf8)).
:- use_module(library(assoc)).
:- use_module(library(pairs)).
:- use_module(library(apply)).
:- use_module(library(lists)).
/** <module> The query protocol's tagged cells (docs/design.md § Query protocol)

A message is a header word, a request number, and cells, all 32-bit words
in big-endian byte order. The header holds the type (top 8 bits), a reserved
field (8 bits, zero) and the number of words that follow it (16 bits).

A cell is a 3-bit tag and a 29-bit value:

| Tag       | Value                                  | Followed by               |
| VAR    0  | the variable's number in the message   | nothing                   |
| INT    1  | a signed 29-bit integer                | nothing                   |
| ATOM   2  | the atom's number                      | nothing                   |
| STRUCT 3  | arity (8 bits), functor's number (21)  | its arguments             |
| BLOB   4  | length in bytes                        | the bytes, four to a word |

A BLOB's first byte names what it holds: `s` a string (UTF-8), `i` an
integer too large for INT (decimal text), `f` a float (its shortest decimal
text that reads back exactly).

Atoms 0 and 1 are fixed: `[]` and `.`; a list cell is `.`/2. The server
numbers every other atom and names each to the client (ATOM_IS: number and
name) before it uses it. SWI-Prolog's list cell '[|]'/2 is `.`/2 here; its
plain atom '[]' and compound '.'/2, which would clash with them, are refused,
and so is a STRUCT named `[|]` of arity 2, which SWI-Prolog cannot tell from
its list cell.
*/

message_type(query,   1).
message_type(next,    2).
message_type(cancel,  3).
message_type(assert,  4).
message_type(retract, 5).
message_type(atom,    6).
message_type(answer,  7).
message_type(done,    8).
message_type(atom_is, 9).

tag(var, 0).
tag(int, 1).
tag(atom, 2).
tag(struct, 3).
tag(blob, 4).

cell(Tag, Value, Cell) :- tag(Tag, T), Cell is (T << 29) \/ Value.

%!  read_message(+In, -Type, -Req, -Body) is semidet.
%   Fails at end of input. Throws on a malformed header.

read_message(In, Type, Req, Body) :-
    read_word(In, Header),
    Header \== end_of_file,
    Code is Header >> 24,
    Reserved is (Header >> 16) /\ 0xff,
    Len is Header /\ 0xffff,
    (   Reserved =:= 0 -> true ; throw(protocol_error(reserved_not_zero(Reserved))) ),
    (   Len >= 1 -> true ; throw(protocol_error(no_request_number)) ),
    (   message_type(Type, Code) -> true ; throw(protocol_error(unknown_type(Code))) ),
    read_word(In, Req),
    N is Len - 1,
    length(Body, N),
    maplist(read_word(In), Body),
    (   memberchk(end_of_file, [Req|Body]) -> throw(protocol_error(message_cut_short)) ; true ).

%   end_of_file only when input ends before a word starts; a word cut short
%   is an error.
read_word(In, W) :-
    get_byte(In, B0),
    (   B0 =:= -1
    ->  W = end_of_file
    ;   get_byte(In, B1), get_byte(In, B2), get_byte(In, B3),
        (   B3 =:= -1 -> throw(protocol_error(word_cut_short))
        ;   W is (B0 << 24) \/ (B1 << 16) \/ (B2 << 8) \/ B3
        )
    ).

write_message(Out, Type, Req, Body) :-
    message_type(Type, Code),
    length(Body, N),
    Len is N + 1,
    (   Len =< 0xffff -> true ; throw(protocol_error(message_too_long(Len))) ),
    Header is (Code << 24) \/ Len,
    maplist(write_word(Out), [Header, Req|Body]),
    flush_output(Out).

write_word(Out, W) :-
    B0 is (W >> 24) /\ 0xff, B1 is (W >> 16) /\ 0xff,
    B2 is (W >> 8) /\ 0xff, B3 is W /\ 0xff,
    put_byte(Out, B0), put_byte(Out, B1), put_byte(Out, B2), put_byte(Out, B3).

		 /*******************************
		 *            ATOMS             *
		 *******************************/

:- dynamic atom_no/2, no_atom/2, next_atom/1.

atom_no('[]', 0).
atom_no('.', 1).
no_atom(0, '[]').
no_atom(1, '.').
next_atom(2).

%!  reset_atoms is det.
%   Forget every atom number but the fixed ones, as at the start.

reset_atoms :-
    retractall(atom_no(_, _)), retractall(no_atom(_, _)), retractall(next_atom(_)),
    assertz(atom_no('[]', 0)), assertz(atom_no('.', 1)),
    assertz(no_atom(0, '[]')), assertz(no_atom(1, '.')),
    assertz(next_atom(2)).

%!  atom_name(?Atom, ?Name) is det.
%   A Prolog atom and its protocol name: [] is "[]"; others are themselves.

atom_name(A, N) :- A == [], !, N = '[]'.
atom_name(A, N) :- atom(A), !, ( A == '[]' -> throw(protocol_error(plain_atom_brackets)) ; N = A ).
atom_name(A, '[]') :- !, A = [].
atom_name(A, N) :- atom(N), A = N.

%!  intern(+Name, -Number, -New) is det.
%   The number of a protocol name; New is true if it was just assigned.

intern(Name, No, New) :-
    (   atom_no(Name, No0)
    ->  No = No0, New = false
    ;   retract(next_atom(No)),
        No1 is No + 1,
        (   No < 1 << 29 -> true ; throw(protocol_error(too_many_atoms)) ),
        assertz(next_atom(No1)),
        assertz(atom_no(Name, No)),
        assertz(no_atom(No, Name)),
        New = true
    ).

		 /*******************************
		 *      TERMS TO CELLS          *
		 *******************************/

%!  terms_cells(+Terms, -Cells, -Vars, -NewAtoms) is det.
%   Encode a list of terms as one cell sequence. Vars lists the variables
%   in the order of their numbers; NewAtoms the names numbered on the way.

terms_cells(Terms, Cells, Vars, NewAtoms) :-
    phrase(seq_cells(Terms, s([], 0, []), s(VarsR, _, NewR)), Cells),
    reverse(VarsR, Vars),
    reverse(NewR, NewAtoms).

seq_cells([], S, S) --> [].
seq_cells([T|Ts], S0, S) --> term_cells(T, S0, S1), seq_cells(Ts, S1, S).

term_cells(T, S0, S) --> { var(T) }, !, var_cell(T, S0, S).
term_cells(T, S0, S0) --> { integer(T) }, !, int_cells(T).
term_cells(T, S0, S0) --> { float(T) }, !, float_cells(T).
term_cells(T, S0, S0) --> { string(T) }, !, { string_utf8(T, Bytes) }, blob_cells(0's, Bytes).
term_cells(T, S0, S) --> { ( T == [] ; atom(T) ) }, !, { atom_name(T, Name), atom_number_new(Name, No, S0, S), cell(atom, No, C) }, [C].
term_cells(T, S0, S) -->
    { compound(T), !,
      compound_name_arity(T, F, A),
      (   F == '[|]', A == 2 -> Name = '.'
      ;   F == '.', A == 2 -> throw(protocol_error(dot_compound))
      ;   A =:= 0 -> throw(protocol_error(zero_arity_compound(F)))
      ;   A > 255 -> throw(protocol_error(arity_too_large(A)))
      ;   atom_name(F, Name)
      ),
      atom_number_new(Name, No, S0, S1),
      (   No < 1 << 21 -> true ; throw(protocol_error(functor_number_too_large(No))) ),
      V is (A << 21) \/ No,
      cell(struct, V, C),
      T =.. [_|Args]
    },
    [C],
    seq_cells(Args, S1, S).
term_cells(T, _, _) --> { throw(protocol_error(cannot_encode(T))) }.

var_cell(T, s(Vs, N, New), S) -->
    (   { var_number(Vs, T, K) }
    ->  { S = s(Vs, N, New) }
    ;   { K = N, N1 is N + 1, S = s([T|Vs], N1, New) }
    ),
    { cell(var, K, C) },
    [C].

var_number(Vs, T, K) :- reverse(Vs, Fwd), nth0(K, Fwd, V), V == T, !.

atom_number_new(Name, No, s(Vs, N, New0), s(Vs, N, New)) :-
    intern(Name, No, IsNew),
    (   IsNew == true -> New = [Name|New0] ; New = New0 ).

int_cells(I) -->
    (   { I >= -(1 << 28), I < 1 << 28 }
    ->  { V is I /\ ((1 << 29) - 1), cell(int, V, C) }, [C]
    ;   { number_codes(I, Codes) }, blob_cells(0'i, Codes)
    ).

float_cells(F) -->
    { float_class(F, Class),
      (   memberchk(Class, [zero, subnormal, normal]) -> true
      ;   throw(protocol_error(non_finite_float(Class)))
      ),
      format(codes(Codes), "~w", [F])
    },
    blob_cells(0'f, Codes).

blob_cells(Kind, Bytes) -->
    { length(Bytes, N0),
      N is N0 + 1,
      (   N < 1 << 29 -> true ; throw(protocol_error(blob_too_long(N))) ),
      cell(blob, N, C),
      Pad is (4 - N mod 4) mod 4,
      length(Zeros, Pad), maplist(=(0), Zeros),
      append([Kind|Bytes], Zeros, All)
    },
    [C],
    words(All).

words([]) --> [].
words([A,B,C,D|T]) --> { W is (A << 24) \/ (B << 16) \/ (C << 8) \/ D }, [W], words(T).

string_utf8(S, Bytes) :-
    string_codes(S, Codes),
    phrase(utf8_codes(Codes), Bytes).

		 /*******************************
		 *      CELLS TO TERMS          *
		 *******************************/

%!  cells_terms(+Cells, -Terms, -Vars) is det.
%   Decode a cell sequence into terms. Vars lists the fresh variables in
%   number order (VAR n is the nth).

cells_terms(Cells, Terms, Vars) :-
    empty_assoc(A0),
    decode_all(Cells, A0, A, Terms),
    assoc_to_list(A, Pairs),
    check_dense(Pairs, 0),
    pairs_values(Pairs, Vars).

check_dense([], _).
check_dense([K-_|T], K) :- !, K1 is K + 1, check_dense(T, K1).
check_dense([K-_|_], E) :- throw(protocol_error(variable_numbers_not_dense(K, E))).

decode_all([], A, A, []) :- !.
decode_all(Cells, A0, A, [T|Ts]) :-
    decode(Cells, Rest, A0, A1, T),
    decode_all(Rest, A1, A, Ts).

decode([], _, _, _, _) :- !, throw(protocol_error(truncated_term)).
decode([C|Rest0], Rest, A0, A, T) :-
    Tag is C >> 29,
    V is C /\ ((1 << 29) - 1),
    (   tag(Name, Tag) -> true ; throw(protocol_error(unknown_tag(Tag))) ),
    decode(Name, V, Rest0, Rest, A0, A, T).

decode(var, K, R, R, A0, A, T) :-
    (   get_assoc(K, A0, T) -> A = A0 ; put_assoc(K, A0, T, A) ).
decode(int, V, R, R, A, A, I) :-
    (   V >= 1 << 28 -> I is V - (1 << 29) ; I = V ).
decode(atom, No, R, R, A, A, T) :-
    number_name(No, Name),
    atom_name(T, Name).
decode(struct, V, R0, R, A0, A, T) :-
    Arity is V >> 21,
    No is V /\ ((1 << 21) - 1),
    (   Arity >= 1 -> true ; throw(protocol_error(zero_arity_struct)) ),
    number_name(No, Name),
    (   Name == '.', Arity =:= 2 -> F = '[|]'
    ;   Name == '[|]', Arity =:= 2 -> throw(protocol_error(list_cell_clash))  % SWI's own list cell
    ;   atom_name(F, Name)
    ),
    length(Args, Arity),
    decode_args(Args, R0, R, A0, A),
    compound_name_arguments(T, F, Args).
decode(blob, N, R0, R, A, A, T) :-
    (   N >= 1 -> true ; throw(protocol_error(empty_blob)) ),
    W is (N + 3) // 4,
    length(Words, W),
    (   append(Words, R, R0) -> true ; throw(protocol_error(truncated_blob)) ),
    words_bytes(Words, Bytes0),
    length(Bytes, N),
    append(Bytes, Pad, Bytes0),
    (   maplist(==(0), Pad) -> true ; throw(protocol_error(blob_padding_not_zero)) ),
    Bytes = [Kind|Data],
    blob_term(Kind, Data, T).

words_bytes([], []).
words_bytes([W|Ws], [A,B,C,D|T]) :-
    A is (W >> 24) /\ 0xff, B is (W >> 16) /\ 0xff,
    C is (W >> 8) /\ 0xff, D is W /\ 0xff,
    words_bytes(Ws, T).

decode_args([], R, R, A, A).
decode_args([X|Xs], R0, R, A0, A) :-
    decode(R0, R1, A0, A1, X),
    decode_args(Xs, R1, R, A1, A).

blob_term(0's, Bytes, S) :- !,
    (   phrase(utf8_codes(Codes), Bytes) -> true ; throw(protocol_error(bad_utf8)) ),
    string_codes(S, Codes).
blob_term(0'i, Codes, I) :- !,
    (   catch(number_codes(I, Codes), _, fail), integer(I) -> true
    ;   throw(protocol_error(bad_integer_text))
    ).
blob_term(0'f, Codes, F) :- !,
    (   catch(number_codes(F0, Codes), _, fail), number(F0)
    ->  F is float(F0)
    ;   throw(protocol_error(bad_float_text))
    ).
blob_term(K, _, _) :- throw(protocol_error(unknown_blob_kind(K))).

number_name(No, Name) :-
    (   no_atom(No, Name) -> true ; throw(protocol_error(unknown_atom_number(No))) ).
