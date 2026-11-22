:- encoding(utf8).
% Tests of prolog/ace_read.pl. Each test says what fault it catches. They
% need APE: run them in the Nix shell, which sets APE_HOME.
% Run: swipl -g run_tests -t halt prolog/tests/test_ace_read.pl

:- use_module('../ace').
:- use_module('../ace_read').
:- use_module(library(plunit)).

:- begin_tests(ace_read).

%   The statements a text reads as, with no user lexicon.
reads(Text, Statements) :-
    ace_lexicon([], _),
    ace_read(Text, read(_, Statements, _)).

test(an_owners_noun_and_its_value_are_a_fact) :-
    % Catches the owner and the value swapped, a possessive read
    % otherwise than "of", or a string kept as an atom.
    reads("The date of Buran-flight is 1988.", [fact(date(buran_flight, 1988))]),
    reads("Buran-flight's date is 1988.", [fact(date(buran_flight, 1988))]),
    reads("The label of Buran-flight is \"Buran's flight\".", [fact(label(buran_flight, L))]),
    assertion(string(L)).

test(a_noun_with_no_owner_is_a_box) :-
    % Catches "the income is 2000" read as a fact of one place, or not
    % at all: a box is box(Name, Value), as the boxes of the tutorial.
    reads("The income is 2000.", [fact(box(income, 2000))]).

test(a_noun_that_is_not_a_value_is_a_kind) :-
    % Catches a kind read as a box ("Nemo is a fish" as box(fish, ...)),
    % a name left as APE gives it (named('Nemo')), or a kind lost in a
    % rule or a constraint.
    reads("Nemo is a fish.", [fact(fish(nemo))]),
    reads("Every fish is an animal.", [rule(animal(X), fish(X))]),
    reads("Every fish swims.", [rule(swim(Y), fish(Y))]),
    reads("No fish is a bird.", [constraint((fish(Z), bird(Z)))]),
    assertion(X-Y-Z == '$VAR'(0)-'$VAR'(0)-'$VAR'(0)).

test(a_rule_concludes_one_thing_of_what_it_names) :-
    % Catches "Every fish has a fin" read as "anything a fish has is a
    % fin" (a conclusion of more than one goal), or a rest of Y that the
    % condition does not name (a head variable the body does not bind).
    ace_lexicon([], _),
    ace_read("Every fish has a fin.", unread(_, unstated, _)),
    ace_read("If the income is X then the rest is Y.", unread(_, unstated, _)).

test(something_with_no_name_is_not_a_fact) :-
    % Catches "A fish is an animal", which ACE reads as one fish with no
    % name, stated as a fact with a variable in it, or not told apart
    % from a text with a part the notebook cannot state.
    ace_lexicon([], _),
    ace_read("A fish is an animal.", unread(_, unnamed, _)).

test(an_adjective_and_a_verb_are_facts_of_their_entities) :-
    % Catches a proper name kept with its capitals or hyphens, or a verb's
    % subject and object swapped.
    reads("Buran is a:reusable.", [fact(reusable(buran))]),
    reads("Buran-flight follows Energia-launch.", [fact(follow(buran_flight, energia_launch))]).

test(a_source_that_says_something_makes_claims_not_facts) :-
    % Catches what a source says taken as the user's own belief.
    reads("Talk-notes says that the date of Buran-flight is 1989.",
          [claim("Talk-notes", date(buran_flight, 1989))]).

test(if_then_a_value_is_a_rule) :-
    % Catches the head's value left unbound, or the sum not computed.
    reads("If the income is X and the rent is Y then the rest is X - Y.",
          [rule(rest(R), (box(income, A), box(rent, B), R is A - B))]),
    assertion(R-A-B == '$VAR'(0)-'$VAR'(1)-'$VAR'(2)).

test(what_must_not_be_is_a_constraint) :-
    % Catches a comparison that must hold read as one that must not, a
    % negation lost, or "more than 1" not read as two different values.
    reads("If the rent is X and the food is Y and the income is Z then X + Y =< Z.",
          [constraint((box(rent, A), box(food, B), box(income, C), \+ A + B =< C))]),
    assertion(A-B-C == '$VAR'(0)-'$VAR'(1)-'$VAR'(2)),
    reads("It is false that the rent is greater than the income.",
          [constraint((box(rent, R), box(income, I), R > I))]),
    assertion(R-I == '$VAR'(0)-'$VAR'(1)),
    reads("Nothing has more than 1 date.", [constraint((date(E, D1), date(E, D2), D1 \== D2))]),
    assertion(E-D1-D2 == '$VAR'(0)-'$VAR'(1)-'$VAR'(2)).

test(a_box_holds_one_value) :-
    % Catches "value" read as a relation value(E, V), which no box is:
    % a box's value is the second place of box(Name, Value), so "Nothing
    % has more than 1 value" is the constraint that a box holds one.
    reads("Nothing has more than 1 value.", [constraint((box(B, V1), box(B, V2), V1 \== V2))]),
    assertion(B-V1-V2 == '$VAR'(0)-'$VAR'(1)-'$VAR'(2)).

test(sentences_each_give_their_statements) :-
    % Catches a text of two sentences read as its first alone, or two
    % equal values given to one owner: each noun keeps its own owner.
    reads("The date of Buran-flight is 1988. The date of Energia-launch is 1987.",
          [fact(date(buran_flight, 1988)), fact(date(energia_launch, 1987))]),
    reads("The date of Buran-flight is 1988. The date of Energia-launch is 1988.",
          [fact(date(buran_flight, 1988)), fact(date(energia_launch, 1988))]).

test(a_name_the_notebook_gives_comes_back_as_it_is) :-
    % Catches a notebook's entity renamed on the way back: the lexicon
    % names energia as Energia-rocket, so it reads as energia.
    ace_lexicon([pn_sg('Energia-rocket', energia, neutr)], _),
    ace_read("The date of Energia-rocket is 1987.", read(_, [fact(date(energia, 1987))], _)).

test(ace_that_states_nothing_is_unread_and_prose_is_not_ace) :-
    % Catches a reading of part of a text, which would state less than
    % it says, or prose taken for ACE.
    ace_lexicon([], _),
    ace_read("There is a flight.", unread(_, unnamed, _)),
    ace_read("Buran-flight quickly follows Energia-launch.", unread(_, unstated, _)),
    ace_read("Buran flew once, unmanned, on 15 November 1988.", not_ace(_)).

test(each_statement_is_written_as_the_command_text_for_it) :-
    % Catches a sum written without its brackets, which reads back as
    % another sum, or a string written without its quotes.
    ace_lexicon([], _),
    ace_read_text("If the income is X and the rent is Y and the food is Z then the rest is X - (Y + Z).",
                  words([], []), read(_, [said(rule, R)], _)),
    assertion(R == "rest(A):-box(income, B), box(rent, C), box(food, D), A is B-(C+D)"),
    ace_read_text("The label of Buran-flight is \"Buran's flight\".", words([], []), read(_, [said(fact, F)], _)),
    assertion(F == "label(buran_flight, \"Buran's flight\")").

test(a_box_a_rule_defines_is_the_rule_value) :-
    % Catches a rule for the box it reads (refused as recursive), or "the
    % rest" read as the box where a rule, of the world or of the same
    % text, defines it, so a constraint on it never fires.
    ace_lexicon([], _),
    ace_read("It is false that the rest is smaller than 0.", words([rest], []), read(_, [W], _)),
    assertion(W == constraint((rest('$VAR'(0)), '$VAR'(0) < 0))),
    ace_read("It is false that the rest is smaller than 0.", words([], []), read(_, [B], _)),
    assertion(B == constraint((box(rest, '$VAR'(0)), '$VAR'(0) < 0))),
    ace_read("If the income is X then the rest is X - 1. It is false that the rest is smaller than 0.",
             read(_, [_, C], _)),
    assertion(C == constraint((rest('$VAR'(2)), '$VAR'(2) < 0))).

test(a_question_asks_for_its_answer_or_whether) :-
    % Catches a question read as a statement, the wrong referent taken
    % for the answer, a plural ("which animals") not read as its kind, or
    % a yes-or-no question given an answer variable.
    reads("What is an animal?", [question('$VAR'(0), animal('$VAR'(0)))]),
    reads("Who swims?", [question('$VAR'(0), swim('$VAR'(0)))]),
    reads("Which animals swim?", [question('$VAR'(0), (animal('$VAR'(0)), swim('$VAR'(0))))]),
    reads("What is the date of Buran-flight?", [question('$VAR'(0), date(buran_flight, '$VAR'(0)))]),
    reads("Is Nemo an animal?", [question(none, animal(nemo))]),
    reads("Does Nemo swim?", [question(none, swim(nemo))]).

test(a_plural_after_are_is_asked_as_its_singular) :-
    % Catches "Which things are animals?" left not ACE (ACE has no plural
    % after "are"), "thing" asked as a kind no fact states, or a statement
    % respelled: "Nemo and Dory are animals" is not one animal.
    reads("Which things are animals?", [question('$VAR'(0), animal('$VAR'(0)))]),
    reads("Which animals are birds?", [question('$VAR'(0), (animal('$VAR'(0)), bird('$VAR'(0))))]),
    ace_lexicon([], _),
    ace_read("Nemo and Dory are animals and Bubbles is a fish.", not_ace(_)),
    ace_read("Which things are blorbs?", not_ace(_)).

test(a_question_of_a_box_asks_for_its_value) :-
    % Catches "What is the income?" read as a kind, income(X), where the
    % world has the box: ACE gives "the" and "an" alike.
    ace_lexicon([], _),
    ace_read("What is the income?", words([], [income]), read(_, [Q], _)),
    assertion(Q == question('$VAR'(0), box(income, '$VAR'(0)))).

:- dynamic animal/1, label/2.
animal(nemo).
animal(dory).

test(a_questions_text_is_a_goal_that_gives_the_answer) :-
    % Catches answers left unsorted, kept twice, or not joined; no answer
    % not said as none; or whether not answered yes or no. The text
    % is run as the memex runs it: read back, then called.
    ace_lexicon([], _),
    ace_read_text("What is an animal?", words([], []), read(_, [said(question, Which)], _)),
    answer_of(Which, A1),
    assertion(A1 == 'dory, nemo'),
    ace_read_text("Is Nemo an animal?", words([], []), read(_, [said(question, Whether)], _)),
    answer_of(Whether, A2),
    assertion(A2 == yes),
    ace_read_text("Is Bubbles an animal?", words([], []), read(_, [said(question, Not)], _)),
    answer_of(Not, A3),
    assertion(A3 == no),
    retractall(animal(_)),
    ace_read_text("What is an animal?", words([], []), read(_, [said(question, None)], _)),
    answer_of(None, A4),
    assertion(A4 == none),
    assertz(animal(nemo)), assertz(animal(dory)).

test(an_answer_is_shown_by_its_label) :-
    % Catches an answer shown as its Prolog name where it has a label, or
    % answers without labels lost, or sorted by kind of term rather than
    % by their text.
    ace_lexicon([], _),
    assertz(label(nemo, "Nemo the clownfish")),
    ace_read_text("What is an animal?", words([], []), read(_, [said(question, Which)], _)),
    answer_of(Which, A),
    retractall(label(_, _)),
    assertion(A == 'Nemo the clownfish, dory').

%   The answer a question's goal text gives, called in this module.
answer_of(Text, Answer) :-
    term_string(Goal, Text, [variable_names(Names)]),
    memberchk('Answer'=Answer, Names),
    call(Goal).

:- end_tests(ace_read).
