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

:- module(ace, [ace_lexicon/2, ace_parse/2, ace_check/2, ace_next/2]).

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
:- use_module(ape(utils/drs_to_drslist), [drs_to_drslist/2]).
:- use_module(ape(utils/drs_to_npace), [drs_to_npace/2]).
:- use_module(ape(utils/drs_to_coreace), [drs_to_coreace/2]).
:- use_module(ape(lexicon/ulex), [add_lexicon_entries/1, discard_ulex/0]).
:- use_module(ape(lexicon/functionwords), [functionword/1]).
:- use_module(ape(lexicon/is_in_lexicon), [is_functionword/1, is_contentword/1]).
:- use_module(ape(lexicon/spellcheck), []).
:- use_module(ape(logger/error_logger), [clear_messages/0, get_messages/1]).
:- use_module(library(time), [call_with_time_limit/2]).
:- use_module(ace_next, [next_words/2, users_word/1]).

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
    % APE leaves a choice point when a word is defined twice.
    once(add_lexicon_entries(Kept)),
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

%!  ace_check(+Text:string, -Check) is det.
%   What a text being typed is, ended with a full stop if it has no end
%   mark: ace(Paraphrase) when it is ACE; open(Next, Completed) when it
%   is not, but can go on as ACE (ace_next.pl): Next the words and kinds
%   of word that may come next, those that the word being typed begins,
%   and Completed the text with that word completed, if one word alone
%   fits ("" if not); else not_ace(Failure, Notes, Repairs). Failure is the first sentence with
%   <> where parsing stopped ("" if APE gave none); Notes are APE's other
%   messages; Repairs are up to three repair(From, To, Mended): a word of
%   Text, a word for its place, and Text so mended, which is ACE.
ace_check(Text, Check) :-
    must_be(string, Text),
    ended(Text, Ended),
    ace_parse(Ended, Result),
    checked(Result, Text, Check).

checked(parsed(Paraphrase, _, _), _, ace(Paraphrase)).
checked(not_ace(_), Text, open(Next, Completed)) :-
    going_on(Text, Next, Completed),
    !.
checked(not_ace(Messages), Text, not_ace(Failure, Notes, Repairs)) :-
    failure(Messages, Failure, Notes),
    repairs(Text, Failure, Notes, Repairs).

%   What may come next in a text that has no end mark: after its last
%   space, filtered by the word being typed, if any; none means it
%   cannot go on. A word being typed is not judged before it ends.
going_on(Text, Next, Completed) :-
    \+ ended_by_mark(Text),
    ace_next(Text, Next),
    Next \== [],
    typing(Text, Done, Typed),
    completed(Done, Typed, Next, Completed).

%!  ace_next(+Text:string, -Next:list) is det.
%   What may come next in a text: the words and kinds of word that may
%   come after its last space (ace_next.pl), those the word being typed
%   begins. With nothing typed, the ways a sentence may begin.
ace_next(Text, Next) :-
    must_be(string, Text),
    typing(Text, Done, Typed),
    next_words(Done, All),
    include(begun_by(Typed), All, Next).

ended_by_mark(Text) :-
    split_string(Text, "", " \t\n", [T]),
    sub_string(T, _, 1, 0, Last),
    sub_atom(".?!", _, 1, _, Last).

%   A text as what is done (up to its last space) and the word being
%   typed after it ("" when it ends with a space).
typing(Text, Done, Typed) :-
    (   sub_string(Text, Before, 1, After, " "),
        \+ ( sub_string(Text, B2, 1, _, " "), B2 > Before )
    ->  sub_string(Text, 0, Before, _, Done0),
        string_concat(Done0, " ", Done),
        sub_string(Text, _, After, 0, Typed)
    ;   Done = "", Typed = Text
    ).

%   A next word the word being typed begins (ignoring case), or a kind of
%   word, which it may begin.
begun_by(_, kind(_)).
begun_by(Typed, own(W)) :- begun_by(Typed, word(W)).
begun_by(Typed, word(W)) :-
    string_lower(Typed, T),
    atom_string(W, S),
    string_lower(S, L),
    string_concat(T, _, L).

%   The text with the word being typed completed, if one word alone fits
%   and something is typed; else "".
completed(Done, Typed, Next, Completed) :-
    (   Typed \== "",
        findall(W, ( member(N, Next), word_of(N, W) ), [W])
    ->  format(string(Completed), "~w~w ", [Done, W])
    ;   Completed = ""
    ).

word_of(word(W), W).
word_of(own(W), W).

%   A text with a full stop after it, unless it ends with . ? or !.
ended(Text, Ended) :-
    split_string(Text, "", " \t\n", [Trimmed]),
    (   sub_string(Trimmed, _, 1, 0, Last), sub_atom(".?!", _, 1, _, Last)
    ->  Ended = Trimmed
    ;   string_concat(Trimmed, ".", Ended)
    ).

%   The message that marks where parsing stopped, and the others.
failure(Messages, Failure, Notes) :-
    (   selectchk(message(error, sentence, _, _, F, _), Messages, Rest)
    ->  Failure = F, Notes = Rest
    ;   Failure = "", Notes = Messages
    ).

%   Up to three edits of one word each that make the text ACE: of the
%   word where parsing stopped, then of each word APE did not know. A
%   check runs at each key typed, and each candidate is a parse, which
%   for a long sentence takes tens of milliseconds: no candidate is
%   parsed after a quarter of a second. (A time limit's exception does
%   not stop it: APE catches exceptions inside its parser.)
repairs(Text, Failure, Notes, Repairs) :-
    split_string(Text, " ", "", Words),
    findall(Target, target(Failure, Notes, Target), Targets),
    get_time(Now),
    Deadline is Now + 0.25,
    findall(R, limit(3, distinct(R, repair(Words, Targets, Deadline, R))), Repairs).

%   An edit of one word that makes the text ACE, tried before a deadline.
repair(Words, Targets, Deadline, repair(From, To, Mended)) :-
    member(At-From, Targets),
    candidate(From, To),
    mended(Words, At-From, To, Mended),
    get_time(Now),
    Now < Deadline,
    ace_or_open(Mended).

%   Whether a text, ended with a full stop if it has none, is ACE; or,
%   if it ends with a space, can go on as ACE ("The flight is ").
ace_or_open(Text) :-
    ended(Text, Ended),
    is_ace(Ended),
    !.
ace_or_open(Text) :-
    typing(Text, Text, ""),
    next_words(Text, [_|_]).

%   Whether APE finds no error in a text: a parse, without its
%   paraphrase, which a repair does not need.
is_ace(Text) :-
    clear_messages,
    call_with_time_limit(5, acetext_to_drs(Text, _, _, _, Found)),
    \+ memberchk(message(error, _, _, _, _), Found).

%   A word to mend, as Index-Word: the index of the word in the text, or
%   0 for its first occurrence.
target(Failure, _, At-Word) :-
    sub_string(Failure, Before, _, _, "<> "),
    sub_string(Failure, 0, Before, _, Prefix),
    split_string(Prefix, " ", " ", Ws),
    exclude(==(""), Ws, Counted),
    length(Counted, N),
    At is N + 1,
    sub_string(Failure, _, _, 0, After0),
    string_concat("<> ", After, After0),
    split_string(After, " ", ".?!,;:", [Word|_]),
    Word \== "".
target(_, Notes, 0-Word) :-
    member(message(error, word, _, _, Word, _), Notes).

%   A word to put in place of another: the present for a past (ACE has no
%   other tense), or a word of the lexicon one edit away.
candidate(From, To) :-
    atom_string(F, From),
    (   present(F, T)
    ;   near(F, T)
    ),
    atom_string(T, To).

%   The words one edit away: the notebook's first.
near(Word, Near) :-
    findall(N, spelled(Word, N), All),
    partition(users_word, All, Users, Others),
    append(Users, Others, Ordered),
    member(Near, Ordered).

present(was, is).
present(were, are).
present(had, has).
present(did, does).
present(Past, Present) :-
    member(End, [ed, d]),
    atom_concat(Stem, End, Past),
    Stem \== '',
    member(S, [s, es]),
    atom_concat(Stem, S, Present),
    is_contentword(Present).

%   A word one edit away (APE's: two letters swapped, one extra, one
%   missing, one wrong) that is a word of ACE.
spelled(Word, Near) :-
    spellcheck:convert_token_char(Word, Chars),
    (   spellcheck:transposition(Chars, New)
    ;   spellcheck:one_letter_extra(Chars, New)
    ;   spellcheck:one_letter_missing(Chars, New)
    ;   spellcheck:one_letter_wrong(Chars, New)
    ),
    spellcheck:convert_token_char(Near, New),
    Near \== Word,
    once(( is_functionword(Near) ; is_contentword(Near) )).

%   The words with one put in place of another: the one at an index, or
%   the first occurrence; its punctuation kept.
mended(Words, At-From, To, Mended) :-
    nth1(I, Words, Word),
    (   At > 0 -> I =:= At ; true ),
    string_concat(From, Rest, Word),
    punctuation(Rest),
    !,
    string_concat(To, Rest, New),
    nth1(I, Words, _, Others),
    nth1(I, NewWords, New, Others),
    atomic_list_concat(NewWords, ' ', Atom),
    atom_string(Atom, Mended).

%   Nothing, or one mark of punctuation.
punctuation("").
punctuation(Mark) :- string_length(Mark, 1), sub_string(".?!,;:", _, 1, _, Mark).

%   APE's paraphrase of a DRS, its sentences joined by spaces.
paraphrase(Drs, Paraphrase) :-
    % APE leaves a choice point here for a text with a question.
    once(drs_to_drslist(Drs, Parts)),
    maplist(restated, Parts, Groups),
    append(Groups, Sentences),
    atomic_list_concat(Sentences, ' ', Atom),
    atom_string(Atom, Paraphrase).

%   A part of a DRS restated, as APE's drs_to_ace/2 does: with relative
%   clauses if that takes at most 50 ms, else in full sentences (core
%   ACE). APE allows 0.5 s, noting that this may take very long "due to a
%   bug?"; for a long sentence it took 0.5 s at each parse (measured),
%   too long for a check at each key typed.
restated(Drs, Ace) :-
    catch(call_with_time_limit(0.05, drs_to_npace(Drs, Ace)), time_limit_exceeded, fail),
    Ace \= [],
    \+ memberchk('ERROR', Ace),
    !.
restated(Drs, Ace) :-
    drs_to_coreace(Drs, Ace),
    Ace \== [],
    !.
%   APE restates no yes-no question ("Does John wait?"): both give
%   nothing, and APE's own paraphrase is "# ...". Say so.
restated(_, ['(APE does not restate this sentence.)']).

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
