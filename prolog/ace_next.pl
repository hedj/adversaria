:- encoding(utf8).
/** <module> What may come next in a sentence of ACE: APE's grammar asked

The predictive editor's lookahead (docs/translator.md, "Do you mean?").
APE's grammar takes each token through a few predicates of
grammar_words (word/3, word_noninitial/4, ...): a function word by its
form, a content word by a condition on it that names its category, such
as noun_sg(W, Lemma, Gender). Each is wrapped here: where the input left
is the marker '$next', the wrapper records what was asked for and fails,
so the grammar goes on to every other way it could continue. Parsing a
text with the marker after it thus finds every word that may come next.
Elsewhere the wrappers do nothing: no text holds the marker.

next_words/2 gives them as own(Form), for the user lexicon's words of
each category asked for (the notebook's words), word(Form), for function
words, and kind(Name), for each category: noun, unit, name, verb,
participle, adjective, adverb, preposition, number, variable.
*/

:- module(ace_next, [next_words/2, users_word/1]).

:- use_module(ape(parser/tokenizer), [tokenize/2]).
:- use_module(ape(parser/ace_to_drs), []).   % loads the grammar (grammar.plp)
:- use_module(ape(parser/grammar_words), [reset_progress_record/1]).
:- use_module(ape(lexicon/ulex), []).
:- use_module(library(time), [call_with_time_limit/2]).

%   The predicates through which APE's grammar takes tokens. The last
%   two arguments of each are the input before and after.
taker(word/3).
taker(word/4).
taker(word_initial/3).
taker(word_initial/4).
taker(word_noninitial/3).
taker(word_noninitial/4).
taker(word_capitalize/4).
taker(words/3).
taker(words/4).
taker(words_initial/3).
taker(words_initial/4).
taker(words_noninitial/3).
taker(words_noninitial/4).

:- forall(taker(Name/Arity),
          ( functor(Head, Name, Arity),
            wrap_predicate(grammar_words:Head, ace_next, Taken,
                           ( asked(Head) -> fail ; Taken )) )).

%   Whether the grammar asks for a token where the marker is, noting
%   what it asks for.
asked(Head) :-
    Head =.. [_|Args],
    append(Wanted, [Input, _], Args),
    at_marker(Input),
    note(Wanted).

at_marker(Input) :-
    nonvar(Input),
    (   Input = [Next|_], Next == '$next'
    ->  true
    ;   Input = [^, Next|_], Next == '$next'
    ).

%   What a taker was asked for: a form, a form and a condition (a content
%   word's category), or a list of forms (the first is what comes next).
note([Form|_]) :- atom(Form), !, seen(word(Form)).
note([_, Condition]) :- compound(Condition), !, functor(Condition, Kind, _), seen(kind(Kind)).
note([[Form|_]|_]) :- atom(Form), !, seen(word(Form)).
note(_).

seen(X) :-
    nb_getval(ace_next_seen, Seen),
    nb_setval(ace_next_seen, [X|Seen]).

%!  next_words(+Text:string, -Next:list) is det.
%   The words and kinds of word that may come after Text, in its last
%   sentence: the user lexicon's words first, then the kinds, then the
%   function words. None if Text cannot go on as ACE, or if the grammar
%   takes more than a quarter of a second.
next_words(Text, Next) :-
    catch(call_with_time_limit(0.25, asked_after(Text, Asked)), _, Asked = []),
    shown(Asked, Next).

%   What the grammar asks for after the last sentence of a text.
asked_after(Text, Asked) :-
    tokenize(Text, Tokens),
    last_sentence(Tokens, Last),
    append([^|Last], ['$next'], Input),
    nb_setval(ace_next_seen, []),
    b_setval(npid, 0),
    reset_progress_record(Input),
    \+ grammar:specification(_, Input, _),
    nb_getval(ace_next_seen, Asked).

%   The tokens after the last end mark.
last_sentence(Tokens, Last) :-
    (   append(_, [End|Last], Tokens), memberchk(End, ['.', ?, !]), \+ ( member(E, Last), memberchk(E, ['.', ?, !]) )
    ->  true
    ;   Last = Tokens
    ).

%   What was asked for, as shown: the user's words of each kind asked
%   for, the kinds, the function words; each once.
shown(Asked, Next) :-
    findall(K, ( member(kind(C), Asked), kind_name(C, K) ), Ks0),
    findall(C, member(kind(C), Asked), Cs0),
    sort(Ks0, Ks),
    sort(Cs0, Cs),
    findall(own(W), ( member(C, Cs), users_word(C, W) ), Users),
    findall(kind(K), member(K, Ks), Kinds),
    findall(word(W), ( member(word(W), Asked), is_word(W) ), Fws0),
    sort(Fws0, Fws),
    append([Users, Kinds, Fws], All),
    list_to_set(All, Next).

%   A form shown as a word: one that starts with a letter.
is_word(W) :- sub_atom(W, 0, 1, _, C), char_type(C, alpha).

%!  users_word(+Word) is semidet.
%   Whether a word is in the user lexicon: a word of the notebook.
users_word(W) :- once(users_word(_, W)).

%   A word of the user lexicon of a category.
users_word(noun_sg, W) :- catch(ulex:noun_sg(W, _, _), _, fail).
users_word(pn_sg, W) :- catch(ulex:pn_sg(W, _, _), _, fail).
users_word(adj_itr, W) :- catch(ulex:adj_itr(W, _), _, fail).

%   A category of APE's lexicon, by the name shown.
kind_name(C, noun) :- memberchk(C, [noun_sg, noun_pl, noun_mass]).
kind_name(C, unit) :- memberchk(C, [mn_sg, mn_pl]).
kind_name(C, name) :- memberchk(C, [pn_sg, pn_pl, pndef_sg, pndef_pl]).
kind_name(C, verb) :- memberchk(C, [iv_finsg, iv_infpl, tv_finsg, tv_infpl, dv_finsg, dv_infpl]).
kind_name(C, participle) :- memberchk(C, [tv_pp, dv_pp]).
kind_name(C, adjective) :- memberchk(C, [adj_itr, adj_itr_comp, adj_itr_sup, adj_tr, adj_tr_comp, adj_tr_sup]).
kind_name(C, adverb) :- memberchk(C, [adv, adv_comp, adv_sup]).
kind_name(prep, preposition).
kind_name(integer, number).
kind_name(variable, variable).
