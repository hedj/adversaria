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
    % at all: a box is cell(Name, Value), as the boxes of the tutorial.
    reads("The income is 2000.", [fact(cell(income, 2000))]).

test(a_noun_that_is_not_a_value_is_a_kind) :-
    % Catches a kind read as a box ("Nemo is a fish" as cell(fish, ...)),
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
          [rule(rest(R), (cell(income, A), cell(rent, B), R is A - B))]),
    assertion(R-A-B == '$VAR'(0)-'$VAR'(1)-'$VAR'(2)).

test(what_must_not_be_is_a_constraint) :-
    % Catches a comparison that must hold read as one that must not, a
    % negation lost, or "more than 1" not read as two different values.
    reads("If the rent is X and the food is Y and the income is Z then X + Y =< Z.",
          [constraint((cell(rent, A), cell(food, B), cell(income, C), \+ A + B =< C))]),
    assertion(A-B-C == '$VAR'(0)-'$VAR'(1)-'$VAR'(2)),
    reads("It is false that the rent is greater than the income.",
          [constraint((cell(rent, R), cell(income, I), R > I))]),
    assertion(R-I == '$VAR'(0)-'$VAR'(1)),
    reads("Nothing has more than 1 date.", [constraint((date(E, D1), date(E, D2), D1 \== D2))]),
    assertion(E-D1-D2 == '$VAR'(0)-'$VAR'(1)-'$VAR'(2)).

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
                  [], read(_, [said(rule, R)], _)),
    assertion(R == "rest(A):-cell(income, B), cell(rent, C), cell(food, D), A is B-(C+D)"),
    ace_read_text("The label of Buran-flight is \"Buran's flight\".", [], read(_, [said(fact, F)], _)),
    assertion(F == "label(buran_flight, \"Buran's flight\")").

test(a_box_a_rule_defines_is_the_rule_value) :-
    % Catches a rule for the box it reads (refused as recursive), or "the
    % rest" read as the box where a rule, of the world or of the same
    % text, defines it, so a constraint on it never fires.
    ace_lexicon([], _),
    ace_read("It is false that the rest is smaller than 0.", [rest], read(_, [W], _)),
    assertion(W == constraint((rest('$VAR'(0)), '$VAR'(0) < 0))),
    ace_read("It is false that the rest is smaller than 0.", [], read(_, [B], _)),
    assertion(B == constraint((cell(rest, '$VAR'(0)), '$VAR'(0) < 0))),
    ace_read("If the income is X then the rest is X - 1. It is false that the rest is smaller than 0.",
             read(_, [_, C], _)),
    assertion(C == constraint((rest('$VAR'(2)), '$VAR'(2) < 0))).

:- end_tests(ace_read).
