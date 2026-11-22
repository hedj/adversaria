# Feature and command usage, learnability, and the novice-to-expert transition (empirical HCI)

How to read the labels. **[primary, read]**: I read the figure in the original paper's text this session (PDF converted to text). **[secondary]**: I read the figure in a paper or summary that cites the original, not in the original. **[search summary]**: the figure came only from a search-engine summary of a page, so treat it as weak. **[logged]** means the study counted from usage logs; **[self-report]** means a survey or an interview; **[lab]** means a controlled experiment. Figures carry their conditions: software, users, method.

## 1. How many commands do users of feature-rich software actually use?

### Takeaway
Across Word, AutoCAD, GIMP and Unix, one person's working vocabulary sits at roughly 30–70 commands. That holds whether the program offers 265 commands or 2,000. Use is very concentrated: in AutoCAD the top 10 commands make up half of all invocations. The overlap between users is small. Only 12–15 commands form a core that most users share; the rest of each person's set is their own.

### Cited Findings
**Microsoft Word 97: McGrenere & Moore (GI 2000), and McGrenere's CHI 2000 poster [primary, read; self-report]**
- Method: 53 people "from the general population", all users of Word 97 on a PC. The sample was not random; the authors took care to cover a range of occupations and organisational levels. Each subject was shown screenshots of every function and asked (1) whether they knew what it did and (2) whether they used it "regularly, irregularly or never". About 1 time in 10 the interviewer asked the subject to explain how a function worked, as a reliability check. — [McGrenere, CHI 2000 poster](https://www.dgp.toronto.edu/~joanna/papers/chi2000_poster_paper.pdf)
- Word 97 had **265 first-level functions**: every icon and final menu item in the default interface. First-level dialog boxes add **another 709**. — [McGrenere 2000](https://www.dgp.toronto.edu/~joanna/papers/chi2000_poster_paper.pdf)
- Mean per participant: **familiar with 135 (51%)**, **ever used 72 (27%)**, **used regularly 40 (15%)**, used irregularly 32 (12%). — [McGrenere 2000](https://www.dgp.toronto.edu/~joanna/papers/chi2000_poster_paper.pdf)
- Ranges: familiar with 24 (9%) to 245 (92%); used **8 (3%) to 119 (45%)**. — [McGrenere 2000](https://www.dgp.toronto.edu/~joanna/papers/chi2000_poster_paper.pdf)
- **42 functions were used by no participant.** Only **12 functions were used regularly by more than 75%** of participants. — [McGrenere 2000](https://www.dgp.toronto.edu/~joanna/papers/chi2000_poster_paper.pdf)
- Attitudes (n=53, counts of agree / no opinion / disagree). "I am overwhelmed by how much 'stuff' there is": 14 / 20 / 17. "I want only the functions I use": 13 / 5 / 35. "I prefer to have unused functions tucked away": 24 / 8 / 21. So 25% wanted unused functions removed, and 45% preferred them tucked away. No participant ever used the word "bloat". — [McGrenere 2000](https://www.dgp.toronto.edu/~joanna/papers/chi2000_poster_paper.pdf)
- The answers did not depend on how many functions a person used or knew, or on their expertise. No group of functions displeased every participant. The authors call this "subjective bloat": unwanted functions that differ from user to user. — [McGrenere 2000](https://www.dgp.toronto.edu/~joanna/papers/chi2000_poster_paper.pdf)

**Microsoft Word 6: Linton et al., OWL (MITRE) [primary, read for method and qualitative findings; secondary for counts; logged]**
- OWL logged each Word command with a timestamp. The study was a year-long naturalistic inquiry. Figure 1 covers **16 users**. — [Linton, Charron & Joy 1998, AAAI WS-98-08](https://cdn.aaai.org/Workshops/1998/WS-98-08/WS98-08-017.pdf)
- **Acquisition order.** Users tend to learn the Edit commands in a set sequence: "those that know fewer commands know a subset of the commands used by their more-knowledgeable peers." The sequence follows the commands' frequency of use. "The more-knowledgeable users do not replace commands learned early on with more powerful commands, but instead keep adding new commands to their repertoire." — [Linton et al. 1998](https://cdn.aaai.org/Workshops/1998/WS-98-08/WS98-08-017.pdf)
- OWL's thresholds were tuned by trial "so as to limit the amount of recommended learning, on average, to two New commands, one More command, and one Alt command" at a time. This is a design choice, not a measured limit. — [Linton et al. 1998](https://cdn.aaai.org/Workshops/1998/WS-98-08/WS98-08-017.pdf)
- Linton et al. quote another study: users apply "less than 10% of Microsoft Word's commands". They do not name the study in the passage I read. — [Linton et al. 1998](https://cdn.aaai.org/Workshops/1998/WS-98-08/WS98-08-017.pdf)
- Counts, **[secondary]**: 16 users of Word 6.0, logged for 3 to 11 months each (mean 6 months). 152 commands observed out of 642 available. The size of a user's vocabulary is "a poor measure of expertise", because it correlates strongly with how long the user was observed. — [Lafreniere et al. 2009, related work](https://cs.uwaterloo.ca/research/tr/2009/CS-2009-27.pdf). **Conflict:** the same paper calls 152 "an average" per user in one place and the community total ("observed only 152 of an available 642 commands") in another. I could not resolve this from the original; the scanned table in the 1998 paper does not convert to text.

**AutoCAD: Matejka, Li, Grossman & Fitzmaurice, CommunityCommands (UIST 2009) [primary, read; logged]**
- Data set: **40 million {User, Command, Time} tuples from 16,000 AutoCAD users over 6 months**, from Autodesk's Customer Involvement Program. — [Matejka et al. 2009](https://research.autodesk.com/app/uploads/2023/03/communitycommands-command-recommendations-for.pdf_recZiyrKCEIuGQLZh.pdf)
- "Even with the thousands of commands available in AutoCAD, the largest group of users only use **between 31 and 40** of them" (histogram, Figure 1). The command count has grown "linearly over time". — [Matejka et al. 2009](https://research.autodesk.com/app/uploads/2023/03/communitycommands-command-recommendations-for.pdf_recZiyrKCEIuGQLZh.pdf)
- Concentration over about 2,000 AutoCAD commands: "the **top 10 commands make up 50%** of all commands issued, and the **top 100 commands make up 93%**." — [Matejka et al. 2009](https://research.autodesk.com/app/uploads/2023/03/communitycommands-command-recommendations-for.pdf_recZiyrKCEIuGQLZh.pdf)
- Users misjudge their own use. Seventeen professional users completed a 3-month field study. All but one underestimated how many commands they used, and "on average our participants use more than 300% as many commands as they estimated" (scripts may explain part of this). They estimated AutoCAD's size at **367 commands on average, about 1/5 of the real count**. — [Matejka et al. 2009](https://research.autodesk.com/app/uploads/2023/03/communitycommands-command-recommendations-for.pdf_recZiyrKCEIuGQLZh.pdf)
- Recommendation quality ("good" = new to the user, and the user says they will use it): 14.7% for Linton's algorithm, 27.2% for user-based collaborative filtering, 30.9% for item-based. "Poor" recommendations: 41.9%, 32.4% and 22.1%. So even the best algorithm's suggestions were adopted in intent less than a third of the time. — [Matejka et al. 2009](https://research.autodesk.com/app/uploads/2023/03/communitycommands-command-recommendations-for.pdf_recZiyrKCEIuGQLZh.pdf)
- One field participant knew a recommended command (CANNOSCALE) was valuable, but had to wait to use it until the team changed its project conventions. Adoption can depend on a team's workflow, not just on awareness. — [Matejka et al. 2009](https://research.autodesk.com/app/uploads/2023/03/communitycommands-command-recommendations-for.pdf_recZiyrKCEIuGQLZh.pdf)
- Deployment, **[search summary]**: a later plug-in was used by more than 1,100 AutoCAD users over a year and gave over 55,000 recommendations. The TOCHI 2011 follow-up reports that the number of unique commands users issued rose significantly. — [Li et al., AAAI/IAAI 2014](https://ojs.aaai.org/index.php/AAAI/article/view/19034); [AI Magazine 2015](https://aaai.org/ojs/index.php/aimagazine/article/view/2600). I did not read these figures in the papers.

**GIMP: Lafreniere, Bunt, Whissell, Clarke & Terry, ingimp (Waterloo TR CS-2009-27) [primary, read; logged]**
- Data set: more than 200 users and 4,000 sessions over 2 years. Analyses use the 22% of users who were "significant" and who produced 75% of the logs. — [Lafreniere et al. 2009](https://cs.uwaterloo.ca/research/tr/2009/CS-2009-27.pdf)
- **487,308 invocations of 352 distinct commands, out of about 500 available.** — [Lafreniere et al. 2009](https://cs.uwaterloo.ca/research/tr/2009/CS-2009-27.pdf)
- **Per-user vocabulary: 1 to 169 commands, mean 34, median 27 (SD 27.9)**, which is under 7% of those available. — [Lafreniere et al. 2009](https://cs.uwaterloo.ca/research/tr/2009/CS-2009-27.pdf)
- **Per session: mean 9 distinct commands, median 6 (SD 9.6).** Median active use per session was 6 minutes (mean 16). — [Lafreniere et al. 2009](https://cs.uwaterloo.ca/research/tr/2009/CS-2009-27.pdf)
- Overlap: no command was used by all users. Undo was used by 91%, Select None by 77.9%. **Only 15 commands were used by more than half the users.** **257 commands (73% of those observed) were each used by no more than 10% of users.** — [Lafreniere et al. 2009](https://cs.uwaterloo.ca/research/tr/2009/CS-2009-27.pdf)
- 31% of sessions (1,288) put no command on the undo stack. — [Lafreniere et al. 2009](https://cs.uwaterloo.ca/research/tr/2009/CS-2009-27.pdf)

**Unix and other command-line studies [secondary unless marked]**
- Greenberg studied 168 csh users in four groups: novice programmers, experienced programmers, computer scientists and non-programmers. — [Greenberg 1988 data set, U. Calgary](https://prism.ucalgary.ca/items/fa341302-c8c9-47e5-9e7d-4417d6628885)
- Greenberg found that each user used **"an average of 50 of the 1307 unique commands observed"**, and that **"less than 3% of commands [were] shared by more than 50% of users."** — [Lafreniere et al. 2009 summarising Greenberg 1993](https://cs.uwaterloo.ca/research/tr/2009/CS-2009-27.pdf)
- Recurrence: "On average, three out of every four command lines entered by the user have already appeared previously" (Unix csh). — [Greenberg 1993, *The Computer User as Toolsmith*, chapter "Principles, corroboration, and justification"](https://www.cambridge.org/core/books/computer-user-as-toolsmith/principles-corroboration-and-justification/03C59031DBDD6F83E1BA58714DBF28CA) **[search summary]**; [Greenberg & Witten, "How users repeat their actions on computers"](https://prism.ucalgary.ca/handle/1880/45927)
- Hanson, Kraut & Farber (1984) saw about 400 of "well over 400" Unix commands in use across the whole community. — [Lafreniere et al. 2009](https://cs.uwaterloo.ca/research/tr/2009/CS-2009-27.pdf)
- Murphy et al. logged 41 developers in Eclipse 3.1/3.2 for 6 to 125 days. Most of them used third-party plug-ins. — [Lafreniere et al. 2009](https://cs.uwaterloo.ca/research/tr/2009/CS-2009-27.pdf)
- Word processors with 6 secretaries and 8 knowledge workers: 50% of keystrokes were text entry and 25% cursor movement. — [Lafreniere et al. 2009, citing ref. 19](https://cs.uwaterloo.ca/research/tr/2009/CS-2009-27.pdf)
- Shape of the distribution: past work agrees that community-wide command frequencies "follow a long-tailed distribution in which a very small number of commands account for the vast majority of observed command use". Individual vocabularies are "relatively small and idiosyncratic, with little overlap between users." — [Lafreniere et al. 2009](https://cs.uwaterloo.ca/research/tr/2009/CS-2009-27.pdf). Zipf frequency distributions have been observed in Unix command use. — [Thomas 1996, SIGCHI Bulletin](https://homepages.cwi.nl/~steven/sigchi/bulletin/1996.2/Rich-Thomas.html) **[search summary]**

### Inferences
- **The personal vocabulary stays about the same size; the share of the program it covers does not.** Measured per-user sizes: Word 97, 40 regular and 72 ever used (self-report). AutoCAD, modal bin 31–40 (logged). GIMP, mean 34 and median 27 (logged). Unix, mean 50 (logged, secondary). Available commands range from 265 to about 2,000. This suggests the limit lies with the user and the task, not the software. The data are observational, so the cause is inferred, not measured.
- **The shared core is about 12–15 commands.** Word: 12 used regularly by more than 75%. GIMP: 15 used by more than 50%. Unix: under 3% of commands shared by more than half the users. Everything beyond the core is personal to the user's tasks.
- **Within one task the active set is about 6–9 commands** (GIMP per-session median 6, mean 9).
- **People add commands in a fairly fixed order, and they add rather than replace** (Linton). For a notebook this argues for a learning path ordered by expected frequency. It also suggests that "power" commands will not displace basic ones unless the design makes the basic ones redundant.
- **People cannot report their own vocabulary.** AutoCAD users underestimated it by more than 3×. Any self-report count in Adversaria's experiment, such as "which commands do you use?", should be checked against the journal log.
- For a single daily user of a research notebook, an inference from the numbers above: about 10–15 core commands everyone uses on day one; a personal set of about 30–50 after months of daily use; about 6–9 per working session. These are estimates drawn from other kinds of software, not measurements of a notebook.
- Caveat: these studies count *commands*, not *concepts*. A concept, such as "world" or "claim", can support many commands, and one command can need several concepts. The figures bound command sets directly and concept sets only loosely.

### Gaps
- I could not read Linton, Joy & Schaefer (1999/2000, UMUAI) itself. The per-user versus total meaning of "152 of 642" is unresolved (see the conflict above), and so is the often-repeated claim that about 20 commands make up about 80–90% of Word use.
- I did not find Lafreniere et al.'s published AutoCAD/Photoshop command-log work (e.g. their later tutorial and "task-centric" papers); only the GIMP (ingimp) report was found.
- Microsoft's Office telemetry claims (e.g. that a handful of commands such as Paste, Save, Copy and Undo dominate Word use, or that most feature requests were for features that already existed) come from Microsoft staff blogs. I did not find or verify them this session.
- I did not read Greenberg's 1993 book chapter directly. The 50 / 1,307 and 75% recurrence figures are secondary.
- No study found measures retention: whether a command, once adopted, stays in use for months. Kurtenbach & Buxton (Q4) show forgetting after breaks, in 2 users.

## 2. The vocabulary problem (Furnas, Landauer, Gomez & Dumais 1987)

### Takeaway
People rarely pick the same word for the same command. Across five domains, the chance that two people chose the same term was below 0.20 (0.07–0.18). A single name chosen by the designer therefore fails 80–90% of the time for untrained users. Many aliases raise the hit rate, but they also make words ambiguous.

### Cited Findings
- Citation: Furnas, G. W., Landauer, T. K., Gomez, L. M. & Dumais, S. T. (1987). "The Vocabulary Problem in Human-System Communication." *Communications of the ACM* 30(11), 964–971. — [CACM](https://cacm.acm.org/research/the-vocabulary-problem-in-human-system-communication) (the page returned 403 to my fetch; abstract text via [search summary](https://bibbase.org/network/publication/furnas-landauer-gomez-dumais-thevocabularyprobleminhumansystemcommunication-1987))
- Abstract, **[search summary]**: across five application domains, the probability that two people chose the same term was below 0.20. "The popular approach in which access is via one designer's favorite single word will result in 80-90 percent failure rates in many common situations." The proposed remedy is "unlimited aliasing", said to give "several-fold improvements". — [CACM abstract via search](https://cacm.acm.org/research/the-vocabulary-problem-in-human-system-communication)
- **[secondary, read]** Brennan (1996): "the probability of any two people producing the same term for the same command ranged from only .07 to .18". For removing a file, people tried "remove, delete, erase, kill, omit, destroy, lose, change, and even trash". — [Brennan 1996](https://www.cs.columbia.edu/~julia/papers/brennan96.pdf)
- **[secondary, read]** Brennan: "even with as many as 20 synonyms for a single command, the likelihood that two people would choose the same term for that command was only about 80%." Synonyms cause a second problem: "In an application with only 25 commands, the likelihood that two people who chose the same term meant the same command by it was only 15%." — [Brennan 1996](https://www.cs.columbia.edu/~julia/papers/brennan96.pdf)
- **[secondary, read]** Within one conversation, people converge on the same terms ("lexical entrainment"). Between conversations, the chance of choosing the same term for the same object was only 10% (Brennan & Clark 1996). — [Brennan 1996](https://www.cs.columbia.edu/~julia/papers/brennan96.pdf)
- A practical effect seen in the AutoCAD study: in a pilot, users "sometimes claimed to use a command frequently, when we could tell from their data that they did not. This was often due to two different commands sounding similar." — [Matejka et al. 2009](https://research.autodesk.com/app/uploads/2023/03/communitycommands-command-recommendations-for.pdf_recZiyrKCEIuGQLZh.pdf)

### Inferences
- If a command must be *recalled by name*, a single fixed name will be guessed by few new users. For daily use the user learns the name, and recall improves through entrainment. The cost falls mostly on first contact and on rare commands.
- This favours recognition over recall for rare commands (menus, a command list with search). Search should match synonyms and descriptions, not only canonical names. Aliases that map many words to one command help discovery. One word mapped to many commands harms precision, so aliasing should be one-way.
- The 15% figure (two people's shared word meaning the same command, in a 25-command system) warns against near-synonym command names inside one closed vocabulary, e.g. "retract" next to "withdraw".

### Gaps
- I did not read the 1987 paper's own tables (domains, numbers of subjects, aliasing curves). The 80% / 20-synonyms and 15% / 25-commands figures are from Brennan's summary and may be paraphrased.

## 3. Learnability: what it is and how it fails (Grossman et al. 2009; Carroll & Rosson 1987; Carroll's minimalism)

### Takeaway
"Learnability" has two parts: *initial* learnability (first use) and *extended* learnability (improvement over time). The main problems are users not knowing a function exists and not knowing how to move to efficient methods. Think-aloud tests miss both problems. Carroll & Rosson's "paradox of the active user" is that people start working at once and avoid reading or practising. That keeps them on slow methods; Fu & Gray (2004) argue the choice is rational over a short horizon.

### Cited Findings
**Grossman, Fitzmaurice & Attar (CHI 2009) [primary, read]**
- The survey covered 88 papers from 1982 to 2008, and the uses of "learnability" fell into 8 categories. — [Grossman et al. 2009](https://research.autodesk.com/app/uploads/2023/03/a-survey-of-software.pdf_recqWPTEYd6nsP0r8.pdf)
- Two main categories of definition: **Initial Learnability**, "initial performance with the system", and **Extended Learnability**, "change in performance over time". — [Grossman et al. 2009](https://research.autodesk.com/app/uploads/2023/03/a-survey-of-software.pdf_recqWPTEYd6nsP0r8.pdf)
- They list 7 categories of learnability metric. One is directly relevant: "C2. Increase in commands used over certain time interval." — [Grossman et al. 2009](https://research.autodesk.com/app/uploads/2023/03/a-survey-of-software.pdf_recqWPTEYd6nsP0r8.pdf)
- Study: AutoCAD, with undergraduate participants aged 19–30 who had 2 months to 5 years of AutoCAD experience (no complete novices, as AutoCAD is not walk-up-and-use). One AutoCAD expert acted as coach. Mean learnability issues per 15-minute task: **2.8 with think-aloud and 7.55 with the question-suggestion protocol** (Protocol F(1,6) = 48.8, p < 0.001; Experience F(3,6) = 5.12, p < 0.05). — [Grossman et al. 2009](https://research.autodesk.com/app/uploads/2023/03/a-survey-of-software.pdf_recqWPTEYd6nsP0r8.pdf)
- Five categories of issue: **Understanding Task Flow, Awareness of Functionality, Locating Functionality, Understanding Functionality, Transitioning to Efficient Behaviors.** "Awareness was a major issue for the most novice users, and Task Flow was rarely a problem for the expert users." The most novice users had no transition issues. Think-aloud exposed a larger share of locating issues, fewer awareness issues, and no transition issues: users rarely say aloud what they do not know exists. — [Grossman et al. 2009](https://research.autodesk.com/app/uploads/2023/03/a-survey-of-software.pdf_recqWPTEYd6nsP0r8.pdf)
- They cite a finding that users "lose up to 40% of their time due to 'frustrating'" experiences with computers. I did not trace the original source. — [Grossman et al. 2009](https://research.autodesk.com/app/uploads/2023/03/a-survey-of-software.pdf_recqWPTEYd6nsP0r8.pdf)

**Carroll & Rosson (1987), "Paradox of the active user" [secondary / search summary]**
- "In 1987, Carroll & Rosson found that users thwart interactions that require reading manuals and practice, even though this results in long-term inefficiency." Fu & Gray (2004) "revisited the paradox ... claiming that it does not show irrational but rational behavior, albeit with a short-term horizon." — [Oulasvirta thread, summarised by search](https://threadreaderapp.com/thread/1055166206108467200.html)
- The AutoCAD field data agree with this: users underestimated how much of the program they did *not* know (an estimated size of 367 commands against about 2,000). — [Matejka et al. 2009](https://research.autodesk.com/app/uploads/2023/03/communitycommands-command-recommendations-for.pdf_recZiyrKCEIuGQLZh.pdf)

### Inferences
- In daily use of a notebook, the main obstacle after week one will be **awareness**, not locating or understanding. Users will not look for what they do not know exists. A test protocol that offers suggestions, such as a coach, or the notebook's own suggestions, finds about 2.7× as many issues as think-aloud (7.55 against 2.8).
- The experiment's measure of adoption ("people adopt suggestions and keep them") matches Grossman's metric C2. "Increase in commands used over a time interval" can be counted directly from Adversaria's journal.

### Gaps
- I did not find a primary or reliable text for Carroll & Rosson (1987) this session. Its two named biases (the "production bias" and the "assimilation bias", as I recall them) are **not verified**.
- I did not find a source for Carroll's *The Nurnberg Funnel* (1990) minimalism results. The often-cited figure that "minimal manual" learners were about 40% faster than learners using a standard manual, with fewer errors, is **recalled, not verified**.

## 4. Novice to expert: shortcuts, marking menus, dips and ceilings

### Takeaway
People rarely move from menus to faster methods without help. Experienced Word users knew few shortcuts, even for commands they used often. Even advanced users used a shortcut as their main method for only 11 of 26 common commands. Designs that make the expert method a rehearsal of the novice one, as marking menus do, or that push the user toward it (audio feedback, disabled menus), raise expert-method use sharply: by 2× for hotkeys, and to over 90% marks in one marking-menu user. Skill fades after breaks.

### Cited Findings
**Surveys**
- Cockburn, Gutwin, Scarr & Malacria (2014), "Supporting novice to expert transitions in user interfaces", *ACM Computing Surveys* 47(2), 31:1–31:36, DOI 10.1145/2659796. — [citation via search](https://unpaywall.org/10.1145%2F3119928). I did not obtain the text (see Gaps).
- Scarr, Cockburn, Gutwin & Quinn (CHI 2011), "Dips and ceilings: understanding and supporting transitions to expertise in user interfaces", pp. 2741–2750. It develops "a framework of the interface and human factors influencing expertise development". — [researchr](https://researchr.org/publication/ScarrCGQ11) **[search summary]**

**Keyboard shortcuts**
- Lane, Napier, Peres & Sándor (2005), "Hidden costs of graphical user interfaces: failure to make the transition from menus and icon toolbars to keyboard shortcuts", *IJHCI* 18(2), 133–144. — [researchr](https://researchr.org/publication/LaneNPS05). Hendy et al. summarise it: "use of keyboard shortcuts was low in a survey of experienced Microsoft Word users." — [Hendy et al., GI 2010](https://www.cs.ubc.ca/~joanna/papers/GI2010_Hendy.pdf) **[secondary, read]**
- Lane et al. detail, **[weak secondary: mailing-list post]**: in a Word save task, participants reported using the toolbar icon about 50% of the time and the menu about 35%; keyboard equivalents were faster for Copy, Open, Paste and Save. — [GNOME desktop-devel list, Oct 2005](https://lists.gnome.org/archives/desktop-devel-list/2005-October/msg00223.html)
- **Hendy, Booth & McGrenere (GI 2010) [primary, read; self-report plus observation]**. 10 advanced users (9 computer-science graduate students and 1 computer-engineering graduate student) were interviewed on Word 2003 about 26 commands taken from Linton, Joy & Schaefer's 1999 list of most-used commands. "Shortcuts are the most commonly used method for only **11 of the 26** commands, even though **21 of the 26** commands have shortcuts available. There were **16 commands where at least half of the participants did not even know the keyboard shortcut**." All 10 ranked shortcuts first in preference and menus last. When asked why they used the mouse, "all participants said that the main reason is 'not knowing the shortcut.'" In observed dialog-box tasks, 40 executions could have been done entirely by keyboard. Shortcuts opened the dialog 16 times, and in 10 of those 16 the user moved to the mouse inside the dialog. Mouse methods were used for "nearly half of the frequently used commands". — [Hendy et al. 2010](https://www.cs.ubc.ca/~joanna/papers/GI2010_Hendy.pdf)
- **Grossman, Dragicevic & Balakrishnan (CHI 2007) [primary, read; lab]**. Users "commonly ignore the displayed hotkey label since once the menu is open they already have access to their desired command." Two techniques, audio feedback on each menu selection and disabling menu selection, "increase hotkey use by over 100%, and reduce task completion times" in the formal experiment. Pilot hotkey use by condition: audio 70%, disabled 69.2%, delayed 59.1%, control 57.1%, visual 50%. The pilot had five participants. — [Grossman et al. 2007](https://www.dgp.toronto.edu/~ravin/papers/chi2007_learninghotkeys.pdf)

**Marking menus: Kurtenbach & Buxton (CHI 1994) [primary, read; field log, n=2]**
- The study logged two users of the ConEd editor, who made 5,237 selections over about 18 hours. User A edited for 8.55 hours over about 6 days, user B for 10.1 hours over 29 days. — [Kurtenbach & Buxton 1994](https://research.autodesk.com/app/uploads/2023/03/user-learning-and-performance.pdf_recFBbPwmG7SW9uDe.pdf)
- "Using a mark on average was **3.5 times faster** than selection using the menu." — [Kurtenbach & Buxton 1994](https://research.autodesk.com/app/uploads/2023/03/user-learning-and-performance.pdf_recFBbPwmG7SW9uDe.pdf)
- User A used the menu for **6.6% of 3,013 selections**, so about 93% were marks. For user B (1,945 selections), the text-converted PDF reads "457. used the menu". This is an OCR error, probably "45%"; **treat it as uncertain**. User B's use of marks "rises dramatically at approximately 650 selections". — [Kurtenbach & Buxton 1994](https://research.autodesk.com/app/uploads/2023/03/user-learning-and-performance.pdf_recFBbPwmG7SW9uDe.pdf)
- **Forgetting:** "when users returned to using ConEd after a lay-off period, the percentage of marking dropped ... mark/command associations were forgotten when not practiced. However, the amount of fading reduced with the amount of experience." "The transition to using marks is not one way. Expert users still switch back to menus to refresh their memory of menu layout." — [Kurtenbach & Buxton 1994](https://research.autodesk.com/app/uploads/2023/03/user-learning-and-performance.pdf_recFBbPwmG7SW9uDe.pdf)
- A secondary critique notes the small sample: "only on 2 users!" — [Berkeley course summary](https://people.eecs.berkeley.edu/~fox/summaries/ui/marking_menus.html) **[search summary]**

### Inferences
- Showing a shortcut label next to a menu item is not enough: users ignore it (Grossman 2007), and even advanced users do not know shortcuts for most commands (Hendy 2010). A notebook that wants keyboard fluency must either make the novice method rehearse the expert one (as a marking menu does, or a command palette that shows and accepts the key) or nudge the user toward it.
- Daily use matters for retention. Skill with marks faded after breaks, and less so with experience. A researcher who uses the notebook daily is the best case. Rare commands (used monthly) behave as if the user were always "after a lay-off", so they should stay reachable by recognition.
- The figures suggest a two-tier design: a small set of high-frequency commands with fast expert methods, and a large tail reached by searchable recognition. This mirrors the concentration in Q1 (in AutoCAD, top 10 = 50% and top 100 = 93% of use).

### Gaps
- I did not obtain the text of Cockburn et al. (2014) or Scarr et al. (2011). Their framework, as I recall it, is **not verified**: three performance domains (intramodal improvement, intermodal improvement, vocabulary extension), plus the "performance dip" when switching to an expert modality and the "ceiling" of a modality.
- I did not read the original Lane et al. (2005) figures, such as the share of frequent users who still used menus.
- I found no study of how many shortcuts experts actually use in daily life, measured by logging.

## 5. Hick–Hyman law and menu breadth: what they imply for the number of choices

### Takeaway
Menu studies with 64 items favour broad, shallow structures: 8×8 beat 2⁶, 4³ and a flat 64. The extra time of a broader screen is outweighed by the levels saved. These results concern *finding an item in a menu*. They say nothing about how many commands a person can learn or keep, so they do not justify "7±2"-style caps on a feature set.

### Cited Findings
- Miller (1981) compared four structures of 64 items: 2⁶, 4³, 8², and 64¹. **[secondary]** "The 8² condition allowed the fastest acquisition and fewest errors of the four structures." — [Hochheiser & Shneiderman 2000](https://www.cs.umd.edu/users/ben/papers/Hochheiser2000Performance.pdf); [Human Factors International newsletter](https://www.humanfactors.com/newsletters/breadth_vs_depth_we_revisit_this_question.html) **[search summary]**
- Kiger (1984), "The depth/breadth tradeoff in the design of menu-driven interfaces": "the data seem to indicate both preference and performance advantage for broad, shallow trees." — [search summary of secondary sources](https://www.humanfactors.com/newsletters/breadth_vs_depth.asp)
- Landauer & Nachbar (1985) **[secondary]**: time per screen rose with the number of alternatives, "but total search time decreased with higher numbers of alternatives per screen because there were fewer levels of depth." — [search summary](https://www.humanfactors.com/newsletters/breadth_vs_depth_we_revisit_this_question.html). **Conflict:** the summary says time per screen rose "linearly". My recollection is that Landauer & Nachbar fitted a *logarithmic* (Hick–Hyman-like) rise for ordered items. I have not verified either.
- Summary claim: the breadth/depth research "suggests that it is better to have more items at each level, and fewer levels." — [Human Factors International](https://www.humanfactors.com/newsletters/breadth_vs_depth_we_revisit_this_question.html) **[search summary]**
- On "7±2": a Dr. Dobb's article argues that applying Miller's 1956 span to menu or interface size is a myth. — [Dr. Dobb's, "The myth of seven plus or minus 2"](https://drdobbs.com/web-development/the-myth-of-seven-plus-or-minus-2/184412300) **[search summary; title only]**

### Inferences
- Breadth studies support showing many items at once (e.g. a full command list grouped into about 8 groups of about 8). They do not support limiting the *number of features* to 7 or to 64. What limits the number a person adopts is the vocabulary data in Q1 (about 30–70 per user), not menu-search time.
- The Hick–Hyman law (decision time grows with the log of the number of equally likely choices) applies to *choices made in the moment, among known alternatives*. With practice and unequal frequencies, the effective cost of a large but skewed command set is much lower than its raw size suggests. This is an inference from the law's form, not from a source read this session.

### Gaps
- I did not read the originals of Hick (1952), Hyman (1953), Miller (1981), Kiger (1984) or Landauer & Nachbar (1985). The figures are secondary.
- I found no source this session for the Hick–Hyman law's application limits in HCI (e.g. Seow 2005, or Cockburn, Gutwin & Greenberg 2007 on menu performance with visual search versus decision).

## 6. Modes, mode errors and consistency

### Takeaway
Mode errors are frequent, and experts are not immune: in Sellen et al.'s vi-style editing task, experts made slightly more mode errors than novices (not significant). A mode the user holds by keeping a pedal pressed (a quasi-mode) cut errors more than a visual mode indicator did. The mode-switching method explained about 11–16% of the variance in errors, and visual feedback about 4–5%. An earlier study cut mode errors by a third with mode-specific sounds.

### Cited Findings
**Sellen, Kurtenbach & Buxton (1992), *Human-Computer Interaction* 7(2), 141–164 [primary, read; lab]**
- 12 expert and 12 novice subjects did a text-editing task. A mode error was a navigation command issued in insert mode, or an attempt to insert text in command mode. The design compared mode switching by keyboard against a foot pedal, each with and without a visual mode indicator. — [Sellen et al. 1992](https://research.autodesk.com/app/uploads/2023/03/the-prevention-of-mode.pdf_recnGhCKDmVQoUNXq.pdf)
- The pedal gave significantly fewer mode errors than the keyboard (liberal criterion F(1,11) = 20.74, p < .001). Visual feedback also reduced errors (F(1,11) = 11.40, p < .003). **Mode-switch method explained 15.6% of the variance (liberal criterion) and 11.0% (conservative); visual feedback explained 4.8% and 4.1%.** — [Sellen et al. 1992](https://research.autodesk.com/app/uploads/2023/03/the-prevention-of-mode.pdf_recnGhCKDmVQoUNXq.pdf)
- "Experts made more errors on average than did novices", though the difference "did not quite reach significance". "Even though many of the expert subjects commented that they were used to keeping track of the mode 'in their head', feedback of both kinds significantly reduced their mode errors nonetheless." — [Sellen et al. 1992](https://research.autodesk.com/app/uploads/2023/03/the-prevention-of-mode.pdf_recnGhCKDmVQoUNXq.pdf)
- For experts, visual feedback helped with keyboard switching but "was redundant in the case of the foot pedal". — [Sellen et al. 1992](https://research.autodesk.com/app/uploads/2023/03/the-prevention-of-mode.pdf_recnGhCKDmVQoUNXq.pdf)
- Experiment 2: the pedal's advantage came from the user having to *hold* insert mode. "User-maintained mode states prevent mode errors more effectively than system-maintained mode states." A non-latching pedal was more salient than a latching one. — [Microsoft Research abstract](https://www.microsoft.com/en-us/research/?p=285995) **[search summary]**
- Monk (1986) "demonstrated that mode errors could be reduced by a third by using a mode-contingent sound with each keystroke." — [Sellen et al. 1992, citing Monk](https://research.autodesk.com/app/uploads/2023/03/the-prevention-of-mode.pdf_recnGhCKDmVQoUNXq.pdf)

### Inferences
- Persistent modes cost errors even for practised users. If the notebook needs a mode, such as "inside a supposed world" or "editing a claim", the evidence favours a mode the user holds or that shows at the point of action, over a status-bar indicator alone. Kinaesthetic feedback beat visual feedback.
- The absolute error rates are in the paper's figures, which did not convert to text. The variance shares are the usable numbers.

### Gaps
- I did not read Raskin's *The Humane Interface* (2000) or Norman's *The Design of Everyday Things* (2013) this session. Their claims, as I recall them, are **not verified**: Raskin on quasimodes, habituation and "monotony", and Norman's taxonomy of slips (capture, description and mode errors). Neither book is a source of measured error rates.
- I found no measured study of how *consistency* (one name and one method for one thing) affects error rates in a feature-rich application.

## 7. Blackwell's attention investment model (2002): when users choose to learn a feature

### Takeaway
Blackwell treats a user's decision to learn or automate as an investment of attention. They weigh the *cost*, the *payoff* (future attention saved) and the *risk* (no payoff, or extra future cost), plus a "prospecting" cost to find out what is possible. Direct manipulation is moderate in cost, low in payoff and low in risk. Programming-like features are high in cost and potentially high in payoff, but high in risk. This explains why users skip features whose payoff is uncertain.

### Cited Findings
- Blackwell, A. F. (2002). "First steps in programming: a rationale for attention investment models." *Proc. IEEE Symposia on Human-Centric Computing Languages and Environments*, 2–10. — [Blackwell 2002 PDF](https://www.cl.cam.ac.uk/~afb21/publications/HCC02a.pdf) **[primary, read]**
- Variables, all in attention units. **Cost**: "attention units to get the work done". **Investment**: "attention units expended toward a potential reduction in future cost". **Pay-off**: "reduced future cost". **Risk**: "probability that no pay-off will result, or even that additional future costs will be incurred". The model adds a **"prospecting" cost**, the cost of "investigating the relative value of alternative courses of action". — [Blackwell 2002](https://www.cl.cam.ac.uk/~afb21/publications/HCC02a.pdf)
- Direct manipulation tends to "involve moderate attentional cost, relatively low payoff and low risk because results can be monitored as the user acts". Programming "tends to involve higher attentional cost ... potentially high payoffs, but also high risks." — [Blackwell 2002](https://www.cl.cam.ac.uk/~afb21/publications/HCC02a.pdf)
- The model is "a decision-theoretic account of programming behavior", proposed to cover "all programming behaviour, from professionals to end-users". It can be checked for small tasks by a cognitive simulation in which one agent at a time holds the "unitary locus of human attention". — [Blackwell 2002](https://www.cl.cam.ac.uk/~afb21/publications/HCC02a.pdf)

### Inferences
- The model and Fu & Gray's reading of the active-user paradox (Q3) say the same thing: users decline to learn when the payoff is uncertain or far away. A notebook feature such as constraints or worlds is adopted when its payoff is visible soon and its risk is low (undoable, previewable). Adversaria's permanent journal raises the felt risk of trying things unless trying is clearly reversible or confined to a supposed world.
- The model gives a test for the experiment: features with the lowest cost of prospecting (visible, previewable effects) should be adopted first. This prediction is mine, not Blackwell's.

### Gaps
- Blackwell's paper is a theoretical model. It gives no measured adoption rates. I found no quantitative test of attention investment in feature adoption this session.

## 8. Objective: what bound do these data put on the feature set one user will adopt and keep?

### Takeaway
The data point to three nested sizes for one daily user, each an estimate drawn from other software: a **shared core of about 10–15 commands**, a **personal working set of about 30–50 commands** after months of use, and **about 6–9 commands within one session**. Adoption beyond the core depends on awareness and reaching the expert method, not on menu layout. Self-reports are unreliable, so the counts must come from logs.

### Cited Findings
- Personal vocabulary: Word 97, mean 40 used regularly and 72 ever used, of 265 ([McGrenere 2000](https://www.dgp.toronto.edu/~joanna/papers/chi2000_poster_paper.pdf)). AutoCAD, modal bin 31–40 of about 2,000 ([Matejka et al. 2009](https://research.autodesk.com/app/uploads/2023/03/communitycommands-command-recommendations-for.pdf_recZiyrKCEIuGQLZh.pdf)). GIMP, mean 34 and median 27 of about 500 ([Lafreniere et al. 2009](https://cs.uwaterloo.ca/research/tr/2009/CS-2009-27.pdf)). Unix, mean 50 of 1,307 observed (secondary, [Lafreniere et al. 2009](https://cs.uwaterloo.ca/research/tr/2009/CS-2009-27.pdf)).
- Shared core: 12 Word functions used regularly by more than 75% ([McGrenere 2000](https://www.dgp.toronto.edu/~joanna/papers/chi2000_poster_paper.pdf)); 15 GIMP commands used by more than 50% ([Lafreniere et al. 2009](https://cs.uwaterloo.ca/research/tr/2009/CS-2009-27.pdf)).
- Per session: median 6, mean 9 GIMP commands ([Lafreniere et al. 2009](https://cs.uwaterloo.ca/research/tr/2009/CS-2009-27.pdf)).
- Concentration: AutoCAD's top 10 commands make up 50% of use, its top 100 make up 93% ([Matejka et al. 2009](https://research.autodesk.com/app/uploads/2023/03/communitycommands-command-recommendations-for.pdf_recZiyrKCEIuGQLZh.pdf)). Unix: 3 of 4 command lines repeat an earlier line ([Greenberg 1993](https://www.cambridge.org/core/books/computer-user-as-toolsmith/principles-corroboration-and-justification/03C59031DBDD6F83E1BA58714DBF28CA), search summary).
- Growth: users add commands in a fairly fixed order that follows frequency, and they keep the early ones ([Linton et al. 1998](https://cdn.aaai.org/Workshops/1998/WS-98-08/WS98-08-017.pdf)). Vocabulary size grows with observation time ([Lafreniere et al. 2009 on Linton](https://cs.uwaterloo.ca/research/tr/2009/CS-2009-27.pdf)).
- Suggestions: at best about 31% of algorithmic recommendations were "good" (new and intended for use) ([Matejka et al. 2009](https://research.autodesk.com/app/uploads/2023/03/communitycommands-command-recommendations-for.pdf_recZiyrKCEIuGQLZh.pdf)). OWL capped suggestions at about 4 at a time ([Linton et al. 1998](https://cdn.aaai.org/Workshops/1998/WS-98-08/WS98-08-017.pdf)).

### Inferences
- **A design budget, an estimate drawn from the above, not a measurement of a notebook:** expect one researcher to use about 10–15 commands on day one, and to reach a stable set of about 30–50 over months. Anything beyond that set is used rarely, so it should be reachable by recognition and search, not by recall. Treat 6–9 commands as the number one session's task needs at once.
- The *available* set can be much larger than the adopted set without harm to most users: only 25% of Word users wanted unused functions removed. But each extra command costs discovery and naming (the vocabulary problem). Mechanisms that compose, so that a few commands cover many cases, are the lever that the counts do not measure directly.
- Bounds on *concepts* need different evidence, such as working-memory limits and studies of mental models. These command counts are an upper bound on the *surface*, not on the conceptual model.
- To test this in Adversaria's experiment, count per user, from the journal: distinct commands per session; distinct commands per week (cumulative); the share covered by the top 10; the commands each suggestion introduced; and whether each was used again after 1, 7 and 30 days. Self-reports should not stand in for any of these counts.

### Gaps
- None of the studies is of a personal knowledge or research notebook. None measures long-term *retention* of adopted commands beyond Kurtenbach & Buxton's two users.
- The sizes come from programs of the 1980s to 2000s with menu/toolbar or command-line interfaces. I found no comparable log study of keyboard-driven, command-palette applications.
