:- encoding(utf8).
/** <module> Attempto Controlled English, through APE

The memex's parser of controlled English (docs/translator.md): APE, with
the Clex lexicon, and a user lexicon the memex sets from the notebook.
APE is found through APE_HOME, which the Nix shell sets.

ace_parse/2 gives, for a text, either its paraphrase, its logical form
(APE's DRS) and APE's messages, or APE's messages alone when the text is
not ACE. Each message is message(Importance, Type, Sentence, Token,
Subject, Repair): Importance is error or warning; Sentence and Token
count from 1, and 0 means none; Subject is what the message is about (a
word, or the sentence with <> where parsing failed); Repair is APE's
advice.
*/

:- module(ace, [ace_lexicon/2, ace_parse/2]).

%   APE's files declare no encoding, and one holds UTF-8 in a comment.
:- set_prolog_flag(encoding, utf8).

:- multifile user:file_search_path/2.
:- dynamic user:file_search_path/2.
:- (   getenv('APE_HOME', Home)
   ->  atom_concat(Home, '/prolog', Dir),
       asserta(user:file_search_path(ape, Dir))
   ;   format(user_error, "prolog/ace.pl: APE_HOME is not set; the Nix shell sets it~n", []),
       halt(3)
   ).

:- use_module(ape(parser/ace_to_drs), [acetext_to_drs/5]).
:- use_module(ape(utils/drs_to_ace), [drs_to_ace/2]).
:- use_module(ape(lexicon/ulex), [add_lexicon_entries/1, discard_ulex/0]).
:- use_module(ape(lexicon/functionwords), [functionword/1]).
:- use_module(ape(logger/error_logger), [clear_messages/0, get_messages/1]).
:- use_module(library(time), [call_with_time_limit/2]).

%!  ace_lexicon(+Entries:list, -Told:list) is det.
%   Make the user lexicon these entries, in APE's lexicon format
%   (pn_sg/3, noun_sg/3, adj_itr/2, ...), less those whose word is one
%   of ACE's function words (the, it, is, ...). Told holds APE's
%   warnings (a word defined twice, say) and a warning for each function
%   word left out. Throws ace_lexicon(Messages) if APE refuses any entry.
%   APE keeps lexicon messages until they are cleared, and a parse would
%   report them as its own, so they are cleared here.
ace_lexicon(Entries, Told) :-
    must_be(list, Entries),
    partition(content_entry, Entries, Kept, Skipped),
    discard_ulex,
    clear_messages,
    add_lexicon_entries(Kept),
    get_messages(Found),
    clear_messages,
    maplist(message, Found, Messages),
    (   memberchk(message(error, _, _, _, _, _), Messages)
    ->  discard_ulex,
        throw(ace_lexicon(Messages))
    ;   maplist(left_out, Skipped, Left),
        append(Messages, Left, Told)
    ).

%   The warning for an entry left out because its word is a function word.
left_out(Entry, message(warning, lexicon, 0, 0, Word, "A function word of ACE: left out of the lexicon.")) :-
    arg(1, Entry, W),
    text(W, Word).

%   An entry whose word is not a function word of ACE.
content_entry(Entry) :-
    compound(Entry),
    arg(1, Entry, Word),
    \+ functionword(Word).

%!  ace_parse(+Text:string, -Result) is det.
%   Result is parsed(Paraphrase, Drs, Messages) when Text is ACE, else
%   not_ace(Messages). The DRS's discourse referents are numbered
%   ('$VAR'(0), '$VAR'(1), ...): the query protocol sends no variable a
%   query did not have. A parse that takes more than 5 seconds throws
%   time_limit_exceeded.
ace_parse(Text, Result) :-
    must_be(string, Text),
    % APE keeps its messages in a global variable, and the server runs
    % each query in an engine of its own, whose global variables start
    % unset: this sets it.
    clear_messages,
    call_with_time_limit(5, acetext_to_drs(Text, _, _, Drs, Found)),
    maplist(message, Found, Messages),
    (   memberchk(message(error, _, _, _, _, _), Messages)
    ->  Result = not_ace(Messages)
    ;   paraphrase(Drs, Paraphrase),
        copy_term(Drs, Numbered),
        numbervars(Numbered, 0, _),
        Result = parsed(Paraphrase, Numbered, Messages)
    ).

%   APE's paraphrase of a DRS, its sentences joined by spaces.
paraphrase(Drs, Paraphrase) :-
    drs_to_ace(Drs, Groups),
    append(Groups, Sentences),
    atomic_list_concat(Sentences, ' ', Atom),
    atom_string(Atom, Paraphrase).

%   A message of APE's, with its place as two numbers (0 for none) and
%   its texts as strings.
message(message(Importance, Type, Sentence-Token, Subject, Repair),
        message(Importance, Type, S, T, Subj, Rep)) :-
    place(Sentence, S),
    place(Token, T),
    text(Subject, Subj),
    text(Repair, Rep).

place(N, N) :- integer(N), !.
place(_, 0).

text(X, S) :- format(string(S), "~w", [X]).
