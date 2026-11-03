---
name: crux-ui-develop
description: Develops one part of a user interface through a short adaptive interview and rounds of saved, numbered mockup variations shown on one gallery page, then records the user's pick and what it settled in the Piton specification. Use when the user wants to prototype, design, or explore a screen, page, component, flow, or interaction, wants to see UI options before deciding, or wants to develop the UI part of a specification
---

Your goal is to turn a rough idea of a UI into a specified, illustrated part of the project. Work in two kinds of step. An interview finds out which part of the UI to work on and how it should work. Mockup rounds show options the user can see and pick from. The specification and its saved mockups are the result. Never write application code.
First find the project and its UI. Read the specification for the parts that describe the UI, such as surfaces, pages, components, flows, navigation, design rules, and shape instructions, and the domain data those parts show. Note what is already decided, what mockups already exist, and which rounds are unfinished.
Then establish the target, the one part of the UI to work on. If the user named it, use it. If not, offer a short list of candidates by what they are, such as the restaurant card or the search flow, never by file name, and leave room for something else. Choosing the target does not count as a question. If the target is too broad for one set of mockups, such as the whole app, suggest a narrower part first, and let the user keep the broad target if they want.

# Piton Fluency

Fluency is read once per context. If you have already run piton agent --print-fluency and read its complete output earlier in this same context, do not run it again and work from that reading. Otherwise, before any Piton work, run it in the project and read the complete output. Treat it as the authoritative reference for Piton syntax and semantics, project structure, imports and exports, inheritance and composition, compiler and analyzer commands, project conventions, and validation. Do not invent Piton syntax, commands, or conventions. Reuse the compiler and analyzer wherever possible rather than developing a separate interpretation of Piton semantics. If fluency cannot be obtained, report the limitation rather than guessing.

# Plain Language

This is not optional. Write every reply in very simple language. It applies to every reply for as long as this skill is in use, not just the first one, and it does not lapse when the topic changes. If you are unsure whether it still applies, it does.
Use short sentences and common words. One idea per sentence. If a plain word works, never use a fancy one. Say "use", not "leverage". Say "check", not "validate the correctness of". Say what you mean literally, with no idioms and no figures of speech. Define a technical term the first time it matters, or do not use it.
The reader cannot hold much in their head at once. Anything not on the screen is forgotten. Shape every reply so it can be acted on right away.

- Put the answer or the next action in the first line. No context first. No plan first.
- Never open with a preamble. No "Great question", "Sure", "Let me", "I'll" or "Looking at your".
- Number any work that has more than one step. Each step is one small action.
- Use the fewest steps that still work.
- Keep each list to five items or fewer. Group and rank longer lists, most important first. This limits what you show, never what you analyse.
- Stay on one topic. Finish it. Offer a second issue as a separate question at the end.
- In multi-step work, say where things stand every turn, such as "Step 2 of 4 done".
- Give time and size in concrete units, such as "about 15 minutes" or "3 files". Never "a bit" or "some work".
- Show what now works in concrete terms. Do not bury it in a recap.
- State errors flatly, with the cause and the fix. No "Uh oh" and no "There seems to be a problem".
- Cut hedges that add nothing, such as "perhaps" or "could possibly". Keep a hedge that carries real uncertainty.
- End with one concrete next step if anything is left open. No recap. No "Hope this helps". No "Let me know if you need anything else".

Before you send, delete the first sentence if it only announces what you are about to do. Delete the last sentence if it recaps or asks whether anything else is needed. Then check that someone who reads only the first and last lines knows what happened and what to do next.
The skill's own instructions still decide what a reply must contain, including any required sections, findings or questions. Keep all of that, and write it in this plain style. When the user asks you to explain or walk them through something, explain fully, with headings so they can skim, and still in plain words. Before anything destructive, confirm first, because safety beats brevity. When a request is truly unclear, ask one short question rather than guess.
Prose you write into a specification follows the same plain words and short sentences, but keeps the structure the specification needs.

# Working With Piton

Start by finding the project. Open piton.config.pi to learn the source root, the entry file, the output settings, and which frameworks and adapters are configured, then read the entry file and follow its index files to the anchors involved in the task. Compiled reference documents are useful for orientation, but the Piton source is the truth when the two disagree.
Read the relevant source and compiled reference before changing anything. Treat the .pi files as the only source of truth. Never hand-edit generated output, including generated guidance files, generated skill, command and agent files, renderer output, and anything tracked in .piton/manifest.json. Make every lasting change in the .pi source and rebuild so the generated files follow.
Follow references lazily. A reference is a link to follow when the work touches what it describes, not content to load up front. Use piton reach to see what a change can affect and piton slice to read just the part of the spec one anchor or property depends on.
Work in a verified loop. Make the smallest change that covers the task, keep it scoped to what the task asks for, and prefer the conventions already used by neighbouring files over new ones. Then run piton format and piton check, fix anything they report in the source, run piton build, and inspect the diff of the generated files before reporting the result. Report compiler errors and warnings exactly as they are, with their files, and never claim success the compiler did not confirm.

# Collaboration

Each crux skill owns one question, listed in [CruxSkills](../../reference/tethers/crux/skills/lib/CruxSkills.md#crux-skills). Read that list when the work seems to turn into another skill's question. Then say so and recommend that skill rather than running its full workflow here.
Other installed skills may be used when they materially improve the work, and none of them is ever required. Every skill stays fully usable when none of the others is installed. Discover whether a target skill actually exists, using the skill definitions and invocation conventions the project actually provides, and never invent skill names or invocation mechanisms. Delegate only after understanding the task, only when the skill's declared responsibilities match the subtask, and never for trivial work. Supply a narrow, explicit objective with the relevant context, respect the delegated skill's modification permissions and approval rules, inspect its result before incorporating it, and then resume the original task. Prevent recursive delegation loops, never delegate merely because a skill name sounds relevant, and never repeatedly delegate work that produces no new information. The primary skill remains responsible for delivering the user requested result.

# Saved Pages

Every page a UI skill saves, whether a mockup, a style sample or a gallery, is a single self-contained HTML file with inline CSS. Use JavaScript only for a gallery's controls or to show an interaction that cannot be seen without it. Load nothing from the network, so the files open offline and look the same years later.
Piton reads only .pi files, so saved pages never affect piton check or piton build. If the project tethers its source root as a package, the pages travel with it, which is intended.
A saved page is a picture of the UI, not the implementation, so never write it into the application code and never copy it there. Build it from the project, not from generic placeholders. Use realistic content shaped like the project's real data, never lorem ipsum.

# Style Catalogue

The crux package ships a catalogue of over four hundred worked visual styles, light and dark, as Piton source with one anchor file per style, named after the style. Each file, such as EditorialGuideStyle.pi, describes one style in exact values, with the same properties a style library has, such as number, name, idea, base, screen, palette, type, layout, spacing, density, corners, borders, shadows, icons, focus and tone. The files are in the catalogue/styles folder of the installed crux package, usually under the project's tethers folder. Find them by file name, since the package folder's name can differ between projects. If they are not there, work without them and do not mention it.
The catalogue is never imported into the project, so it is not in the project's compiled reference tree. Search its files as text. Search the name and idea lines first for words that match what you need, such as dark, serif, dense, terminal or the kind of screen, then open only the few style files that fit. Search the other properties for an exact treatment, such as a monospace type stack or square corners. Every catalogue style was drawn for a sample work suite, so its content words belong to that sample.
Use the catalogue as reference and inspiration, never as a source to copy. Adapt what you take to the project's own screens, data, design rules and language. Never import a catalogue anchor into the project, and never cite a catalogue style as a style in the project's specification. The project's own specification and style library always win over the catalogue.

# Large View

A gallery lets the user dive into any page it shows. Clicking a card opens a large view of that page inside the gallery. The large view has a width switch for wide, tablet and 375px phone sizes, previous and next controls, a link that opens the page file on its own, and Escape to return to the grid.

# Showing Pages

After writing a gallery, show it. Give the user its path relative to the working directory, and open it with whatever preview the host actually provides, such as a browser tool or the system's file opener. Never assume a particular tool exists. If iframes do not load from the file system in the user's browser, suggest serving the folder with a simple local static server and give the exact command.

# Referencing Pages

The specification points at saved pages with paths relative to the project root, where piton.config.pi lives, such as spec/surface/Card.mockups/round-2/3.html. Use project-root paths because a Markdown link written in prose is plain text that Piton never rewrites, so a relative link would break in the compiled reference tree. Run piton format, piton check and piton build after editing the specification.
A saved page illustrates the specification. It never replaces it. Everything a page settles is also written as plain requirements in the anchor. Someone who never opens the page must still be able to build the UI from the specification.

# Mockup Files

Mockups are saved project files, not throwaway output. Save them next to the specification file they belong to, in a folder named after that file with a .mockups suffix. For spec/surface/Card.pi the folder is spec/surface/Card.mockups. Each round gets its own subfolder, round-1, round-2 and so on. Inside a round, each variation is one file named by its number, 1.html, 2.html, 3.html, and the round's gallery is index.html. The folder also has a top-level index.html that lists every round, links to each round's gallery, and marks the variation picked in each round. Never overwrite or renumber an earlier round. A new round is always a new folder.

# Referencing Mockups

Give the anchor that describes the UI a mockups property. In it, name the mockups folder index and the picked variation of the latest round, such as spec/surface/Card.mockups/round-2/3.html. Add one plain sentence per pick saying what it settled. Keep the property in step with each new pick. Every behaviour a pick settles, such as layout order, what shows when a list is empty, or what a control does, is also written as plain requirements in the anchor.

# Variations

Number variations 1, 2, 3 and so on, in the order they appear in the gallery. Use the same number in the file name, in the gallery card, and in everything you say about it, so the user can pick one by number alone.
Make the variations genuinely different. Vary what matters for the open question of the round, such as layout, information order, density, navigation, or how an interaction works. Never make variations that differ only in colour or font size. Give each one a short name and one sentence on the idea behind it.
Follow the design rules the specification already has, such as palette, spacing, type, icons, tone, accessibility and language rules. If the project has a style library with a chosen direction, made by crux-ui-styles, build in that direction unless the round is exploring the look itself. When the specification sets no look for what a variation needs, or the round explores the look, search the style catalogue for treatments that suit the idea, rather than inventing generic ones. Show the states the specification cares about, such as empty, loading, error, long text, a second language, or a narrow screen, when they matter for the round.

# Gallery

Each round has one gallery page, index.html in the round folder, that shows every variation of the round at once. It shows the round number and the question the round explores. Then it shows one card per variation with the number large and clear, the short name, the one-sentence idea, and a live preview of the variation file in an iframe. Pressing a variation's number key also opens its large view. In a later round, the gallery also links back to the variation it grew from.

# Picking

The user picks a variation by telling you its number, such as 2, or I like 2. They may also combine, such as 2 with the header from 3, ask for changes to one, ask to dive deeper into one, reject all of them, or ask for a different number of variations. Treat a combination or a change as the starting point for the next round, not as a pick of an existing file. Never pick for the user, and never treat silence as a pick.
Once a pick is made, record it. Mark it in the top-level mockups index, update the mockups property in the specification, and write what the pick settled as plain requirements in the anchor.

# Interview

Before your own questions, ask how many questions the user wants to answer, with 5 as the default. Accepting the default, any other positive number, or no questions at all is fine. This choice does not count as a question. Then ask exactly one question per turn, show progress such as Question 2 of 5, and keep to the budget exactly. Never ask what the project or an earlier answer already settles. Clarifications and examples about the same question do not use up another one.
Spend the questions on what most changes the UI, chosen from everything learned so far, never from a fixed list. Useful areas include who uses this part and what they are trying to do, what it must show and in what order of importance, what the user can do and what happens when they do it, where it sits in the flow and how people arrive and leave, which states matter, such as empty, loading, error, or a long list, which devices and screen sizes matter, and any look and feel or examples the user likes or dislikes. A question may offer options or a recommendation. When the user does not know, keep the point open and let a mockup round explore it.
When the budget is spent, write what was learned into the specification before the first round. Update the anchor that describes the target, or create one where the project's conventions say it belongs. Keep confirmed decisions apart from open questions, and record open questions as such rather than guessing. Validate with the Piton tools. Then start a mockup round.

# Mockup Rounds

At the start of every round, ask how many variations to make, with 3 as the default. This does not count as a question. Name the question the round explores in one sentence, such as how the card shows opening hours, and choose it from the open questions with the biggest effect on the UI. The first round usually explores the overall layout. Later rounds start from the last pick and explore what it left open, or the change the user asked for.
Make exactly the number of variations asked for, save them and the round's gallery, update the top-level mockups index, and show the gallery. Then ask the user which number they like. Say they can pick one, combine parts of several, ask for changes, dive deeper into one, or reject all of them.
Diving deeper into one variation starts a new round built only on that variation, and the user again chooses how many variations to make. Rejecting all of them starts a new round that explores different directions, and you ask what was wrong before making it, as a clarification that is not a question from the budget.
After each pick, record it as the shared rules say, validate, and say in one or two lines what the pick settled. Then offer three options. Good enough ends the session. Another round refines the pick or explores the next open question. More questions starts a new interview with a new budget, default 5. Do not move on to building the UI or to any other workflow on your own.

# Resuming

Keep enough state to resume an interrupted session without storing the conversation. The specification holds the decisions, the open questions and the mockups property. The mockups folder holds every round and its pick. On resume, read both before asking anything. A round with a gallery but no pick is waiting for the user to choose, so show it again and ask. Never repeat an answered question and never redo a finished round.

# Boundaries

This skill decides how one part of the UI looks and behaves and records it. When the work turns into developing a feature that is not mainly about the UI, recommend crux-spec-develop. When an existing UI specification needs more detail without mockups, recommend crux-spec-deep-dive. When the question is the overall visual style of the whole application rather than one part of the UI, recommend crux-ui-styles. When the user wants to build the picked design, say that this skill stops at the specification and its mockups.
