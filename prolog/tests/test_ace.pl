:- encoding(utf8).
% Tests of prolog/ace.pl. Each test says what fault it catches. They need
% APE: run them in the Nix shell, which sets APE_HOME.
% Run: swipl -g run_tests -t halt prolog/tests/test_ace.pl

:- use_module('../ace').
:- use_module('../ace_next').
:- use_module(library(plunit)).

:- begin_tests(ace).

test(a_sentence_of_ace_gives_its_paraphrase_and_logical_form) :-
    % Catches a parse that gives no paraphrase, or no logical form.
    ace_lexicon([], []),
    ace_parse("John likes Mary.", parsed(P, Drs, Ms)),
    assertion(P == "John likes Mary."),
    assertion(Drs = drs([_|_], [_|_])),
    assertion(ground(Drs)),
    assertion(Ms == []).

test(a_sentence_not_ace_names_where_it_failed) :-
    % Catches a failure without the place parsing stopped, or taken as a
    % parse: ACE has no past tense, so "was" breaks the sentence.
    ace_lexicon([], []),
    ace_parse("The flight was in 1989.", not_ace(Ms)),
    assertion(memberchk(message(error, word, 1, 0, "was", "as"), Ms)),
    assertion(memberchk(message(error, sentence, 1, 3, "The flight <> was in 1989.", _), Ms)).

test(a_word_from_the_notebook_makes_a_sentence_ace) :-
    % Catches a user lexicon not used: buran_flight is in no lexicon
    % until the notebook names it.
    ace_lexicon([], []),
    ace_parse("The date of buran_flight is 1988.", not_ace(_)),
    ace_lexicon([pn_sg(buran_flight, buran_flight, neutr)], []),
    ace_parse("The date of buran_flight is 1988.", parsed(P, _, _)),
    assertion(P == "A date of buran_flight is 1988.").

test(a_new_lexicon_replaces_the_last) :-
    % Catches entries kept from a lexicon set before.
    ace_lexicon([pn_sg(buran_flight, buran_flight, neutr)], []),
    ace_lexicon([], []),
    ace_parse("The date of buran_flight is 1988.", not_ace(_)).

test(a_malformed_entry_is_refused_and_says_why,
     [throws(ace_lexicon([message(error, lexicon, 0, 0, _, _)|_]))]) :-
    % Catches a malformed entry kept silently.
    ace_lexicon([pn_sg(buran_flight)], _).

test(text_that_is_not_a_string_is_refused, [throws(error(type_error(string, _), _))]) :-
    % Catches an atom or a list taken for the text.
    ace_parse('John waits.', _).

test(an_empty_text_is_not_ace) :-
    % Catches the empty text taken as a sentence.
    ace_lexicon([], []),
    ace_parse("", Result),
    assertion(Result \= parsed(_, drs(_, [_|_]), _)).

test(a_function_word_is_left_out_of_the_lexicon) :-
    % Catches a function word kept as a name: APE takes it, and warns that
    % it should not be redefined at every parse after, among the
    % sentence's own messages.
    ace_lexicon([pn_sg(the, the, neutr), pn_sg(food, food, neutr)], Told),
    assertion(Told == [message(warning, lexicon, 0, 0, "the", "A function word of ACE: left out of the lexicon.")]),
    ace_parse("Food waits.", parsed(_, _, Ms)),
    assertion(\+ memberchk(message(_, lexicon, _, _, _, _), Ms)).

test(a_parse_holds_only_its_own_messages) :-
    % Catches messages kept from the parse before: those of a sentence
    % not ACE among those of the next.
    ace_lexicon([], []),
    ace_parse("The flight was in 1989.", not_ace(_)),
    ace_parse("John waits.", parsed(_, _, Ms)),
    assertion(Ms == []).

test(a_word_defined_twice_is_told_once_and_not_at_each_parse) :-
    % Catches a lexicon warning kept by APE and reported by every parse
    % after as the sentence's own: two entities named Buran.
    ace_lexicon([pn_sg('Buran', buran, neutr), pn_sg('Buran', energia, neutr)], Told),
    assertion(Told = [message(warning, lexicon, 0, 0, "Buran", _)]),
    ace_parse("John waits.", parsed(_, _, Ms)),
    assertion(Ms == []).

test(a_parse_runs_in_an_engine_of_its_own) :-
    % Catches APE's messages kept in a global variable that a new engine,
    % as the server runs each query in, does not have: every parse threw
    % existence_error(variable, message).
    ace_lexicon([], []),
    engine_create(R, ace_parse("John waits.", R), E),
    engine_next(E, Result),
    engine_destroy(E),
    assertion(Result = parsed("John waits.", _, [])).

test(setting_the_lexicon_leaves_no_choice_point) :-
    % Catches a choice point APE leaves when a word is defined twice: the
    % server would look for a second answer, and set the lexicon again.
    Twice = [pn_sg('Buran', buran, neutr), pn_sg('Buran', energia, neutr)],
    call_cleanup(ace_lexicon(Twice, _), Det = true),
    assertion(Det == true).

test(a_sentence_of_ace_is_checked_without_its_full_stop) :-
    % Catches a sentence being typed refused only for wanting its full
    % stop.
    ace_lexicon([], []),
    ace_check("John waits", Check),
    assertion(Check == ace("John waits.")).

test(a_past_tense_is_mended_to_the_present) :-
    % Catches no repair for the commonest error: ACE has no past tense,
    % and APE's own advice for "was" is "as".
    ace_lexicon([], []),
    ace_check("The flight was in 1989", not_ace(Failure, _, Repairs)),
    assertion(Failure == "The flight <> was in 1989."),
    assertion(Repairs = [repair("was", "is", "The flight is in 1989")|_]).

test(a_misspelt_word_is_mended_to_the_one_that_parses) :-
    % Catches only APE's first spelling (at for dat) offered, which does
    % not parse, and not date, which does.
    ace_lexicon([pn_sg(buran_flight, buran_flight, neutr)], _),
    ace_check("The dat of buran_flight is 1989.", not_ace(_, _, Repairs)),
    assertion(memberchk(repair("dat", "date", "The date of buran_flight is 1989."), Repairs)),
    assertion(\+ memberchk(repair(_, "at", _), Repairs)).

test(a_regular_past_is_mended_to_its_present) :-
    % Catches the -ed past not mended: happened is happens in ACE.
    ace_lexicon([pn_sg(buran_flight, buran_flight, neutr)], _),
    ace_check("buran_flight happened.", not_ace(_, _, Repairs)),
    assertion(Repairs = [repair("happened", "happens", "buran_flight happens.")|_]).

test(only_the_word_where_parsing_stopped_is_mended) :-
    % Catches a word mended where it first occurs, not where parsing
    % stopped, and a word mended inside a longer one.
    ace_lexicon([], []),
    ace_check("John waits and the man was happy", not_ace(_, _, Repairs)),
    assertion(Repairs = [repair("was", "is", "John waits and the man is happy")|_]),
    ace_check("The washer was red", not_ace(_, _, [repair(_, _, Mended)|_])),
    assertion(Mended == "The washer is red").

test(no_repair_is_offered_when_none_parses) :-
    % Catches a repair offered that does not make the text ACE.
    ace_lexicon([], []),
    ace_check("Qzx vbn.", not_ace(_, Notes, Repairs)),
    assertion(Repairs == []),
    assertion(Notes \== []).

test(a_sentence_ape_cannot_restate_says_so) :-
    % Catches an empty paraphrase: APE restates no yes-no question, and
    % "Does John wait?" was said to be ACE with nothing after; and the
    % choice point APE leaves when it splits a text with a question.
    ace_lexicon([], []),
    call_cleanup(ace_parse("John waits. Does Mary wait?", parsed(P, _, _)), Det = true),
    assertion(Det == true),
    assertion(P == "John waits. (APE does not restate this sentence.)").

test(a_long_sentence_is_paraphrased_within_a_quarter_second) :-
    % Catches APE's paraphraser left to its own half second: with
    % relative clauses it ran out its 0.5 s limit at every parse of this
    % sentence before falling back to full sentences.
    ace_lexicon([], []),
    Long = "If a man owns a dog then the man feeds the dog and the dog barks and the man walks to a park that is green and that is large and that is near a river.",
    get_time(Start),
    ace_parse(Long, parsed(_, _, _)),
    get_time(End),
    assertion(End - Start < 0.25).

test(the_words_that_may_come_next_are_the_grammars) :-
    % Catches a lookahead that misses a function word, a kind of word, or
    % the notebook's words of that kind, or that offers a word the grammar
    % does not take there: after "The date of" comes a name or "the", not
    % a verb.
    ace_lexicon([pn_sg(buran_flight, buran_flight, neutr), adj_itr(reusable, reusable)], _),
    next_words("The date of", Next),
    assertion(memberchk(own(buran_flight), Next)),
    assertion(memberchk(kind(name), Next)),
    assertion(memberchk(word(the), Next)),
    assertion(\+ memberchk(kind(verb), Next)),
    assertion(\+ memberchk(own(reusable), Next)),
    Next = [First|_],
    assertion(First == own(buran_flight)).

test(nothing_may_come_after_a_broken_beginning) :-
    % Catches a text that cannot go on as ACE given next words: ACE has no
    % past tense.
    ace_lexicon([], []),
    next_words("The flight was", Next),
    assertion(Next == []).

test(the_next_words_follow_the_last_sentence) :-
    % Catches a text of two sentences read as one.
    ace_lexicon([], []),
    next_words("John waits. The", Next),
    assertion(memberchk(kind(noun), Next)).

test(a_beginning_of_ace_is_open_with_what_may_come_next) :-
    % Catches a sentence still being typed reported as not ACE.
    ace_lexicon([pn_sg(buran_flight, buran_flight, neutr)], _),
    ace_check("The date of ", open(Next, "")),
    assertion(memberchk(own(buran_flight), Next)).

test(a_word_being_typed_filters_the_next_words_and_one_alone_completes) :-
    % Catches next words not filtered by what is typed, ignoring case, or
    % no completion when one word alone fits.
    ace_lexicon([pn_sg(buran_flight, buran_flight, neutr)], _),
    ace_check("The date of BUR", open(Next, Completed)),
    assertion(\+ memberchk(word(the), Next)),
    assertion(Completed == "The date of buran_flight "),
    ace_check("The date of ", open(_, None)),
    assertion(None == "").

test(a_word_ended_that_breaks_the_sentence_is_mended_to_go_on) :-
    % Catches a repair required to make a sentence still being typed
    % whole: "The flight is" is not a sentence, but it goes on.
    ace_lexicon([], []),
    ace_check("The flight was ", not_ace(_, _, [First|_])),
    assertion(First == repair("was", "is", "The flight is ")).

test(the_notebooks_word_is_offered_first) :-
    % Catches APE's order (dart, data, date) kept when the notebook has
    % date.
    ace_lexicon([noun_sg(date, date, neutr), pn_sg(buran_flight, buran_flight, neutr)], _),
    ace_check("The dat of buran_flight is 1989.", not_ace(_, _, [First|_])),
    assertion(First = repair("dat", "date", _)).

test(a_word_begun_that_two_words_fit_completes_to_neither) :-
    % Catches the first of two fitting words taken as the completion.
    ace_lexicon([pn_sg(buran_flight, buran_flight, neutr), pn_sg(buran_orbit, buran_orbit, neutr)], _),
    ace_check("The date of bur", open(Next, Completed)),
    assertion(memberchk(own(buran_orbit), Next)),
    assertion(Completed == "").

test(what_may_begin_a_sentence_is_listed) :-
    % Catches nothing listed before anything is typed.
    ace_lexicon([pn_sg(buran_flight, buran_flight, neutr)], _),
    ace_next("", Next),
    assertion(memberchk(own(buran_flight), Next)),
    assertion(memberchk(word(every), Next)).

test(what_may_come_after_a_whole_sentence_is_listed) :-
    % Catches a whole sentence given nothing to go on with: "and" may
    % follow "John waits".
    ace_lexicon([], []),
    ace_next("John waits ", Next),
    assertion(memberchk(word(and), Next)).

:- end_tests(ace).
