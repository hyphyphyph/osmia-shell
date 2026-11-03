---
name: crux-ui-styles
description: Builds and grows a library of very different visual styles for an application, such as a clean dashboard, a SaaS product or a technical tool. Three short questions set the directions, then each style is saved as a numbered sample page, shown on one gallery page, and described in the Piton specification so it can guide the look of the application. Use when the user wants to explore visual styles, look and feel, themes or design directions, wants to see very different styles side by side, wants to add more styles to the project's style library, or wants to choose a visual direction for the application
---

Your goal is to grow the project's style library. A style is one complete visual direction for the whole application, such as a clean dashboard, a SaaS product, or a technical tool. Each run asks three questions, adds a small batch of new styles, shows them, and records them in the specification. The library keeps growing over many runs, and the specification uses it to decide the look of the application. Never write application code.
First find the project and its library. Read the specification for what the application is, who uses it, its main screens and the domain data they show, and any design rules it already has, such as palette, type, spacing, icons, tone, accessibility and language rules. Then look for an existing style library anchor and its folder. Note which styles exist, which the user favoured or rejected, and why. If there is no library yet, this run creates it.

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

# Library

The library has one anchor in the specification and one folder of pages beside it. Put the anchor where the project's conventions say design rules belong, and name it after what it holds, such as VisualStyles. If the project has no convention, use a design folder under the source root, such as spec/design/VisualStyles.pi. The folder sits next to the anchor file and is named after it with a .styles suffix, such as spec/design/VisualStyles.styles.
Each style is one page in that folder, named by its number, such as 1.html, 2.html and 7.html. Numbers are global and permanent. A new style takes the next free number. Never renumber, overwrite or delete a style, even a rejected one, because the specification and later runs refer to styles by number. The folder also holds index.html, the gallery of the whole library.

# Questions

Ask exactly three questions, one per turn, and show progress such as Question 2 of 3. Never ask what the project or an earlier answer already settles. When the project settles one of the three, use that turn for the most useful open question about the styles instead. Clarifications and examples about the same question do not use up another one. A question may offer options and a recommendation.
Question 1 is which directions to add. Offer a short list of named directions that the library does not cover yet, such as clean dashboard, SaaS product, technical, editorial, playful, or dense enterprise, and recommend three that fit the application. Search the style catalogue for directions that suit the application and that the library lacks, so the list goes beyond the obvious ones. The user may pick from the list, name their own, or ask you to choose. Make one style per direction agreed, with 3 as the default and 6 as the most in one run.
Question 2 is what to lean toward and away from. Ask for products, sites or moods the user likes or dislikes, and for hard limits, such as light or dark, brand colours, contrast, density, or a second language.
Question 3 is what each style shows. Recommend one representative screen of the application, chosen from the specification, such as the main list or the dashboard home, and ask whether that screen fits or another would show the styles better. Every style in a batch shows the same screen with the same content, so the styles can be compared fairly.

# Style Pages

Each style page shows two things. First, the chosen screen of the application drawn fully in that style. Second, below it, a short style sheet: the colour palette as named swatches with their values, the type scale with the font stacks, spacing and corner radius, and a few basic controls, such as a button, a text field, a table row or a card, in their normal and focus states. Use system font stacks or fonts named by the project, never web fonts, since the page loads nothing from the network.
Make the styles very different from each other and from every style already in the library. Vary what changes the whole feel, such as palette, light or dark base, type, density, layout rhythm, corners, borders, shadows, icon treatment, and tone of the labels. Never make styles that differ only in accent colour or font size. If a new style would come out close to an existing one, change it or say so and suggest another direction. Before drawing a style, read the few catalogue entries closest to its direction for exact, tested values and ideas that suit it, then make the style the project's own. Every style still meets basic accessibility: text has enough contrast, controls have names, and meaning never depends on colour alone.
Give each style a short name and one sentence on the idea behind it, such as Clean dashboard: calm white panels that put the numbers first.

# Style Gallery

The gallery, index.html in the library folder, shows the whole library. It shows one card per style with the number large and clear, the short name, the one-sentence idea, the user's verdict when there is one, and a live preview of the style page in an iframe. It groups styles by the run that made them, newest first, so the latest batch is at the top. It has a filter to show all, favoured, unrated or rejected styles. Typing a style's number and pressing Enter opens its large view.

# Recording

After saving a batch and its gallery, record it in the specification before showing it. The library anchor holds a property that names the gallery path, and one property per style. Each style property gives its number, name, idea, page path and verdict, and then describes the style in plain words and exact values: palette with colour values and what each colour is for, type with font stacks and sizes, spacing, density, corners, borders, shadows, icons, and tone of labels. A builder who never opens the page must be able to apply the style from this description alone.
The anchor also holds a direction property. It says which styles the user favours, which they rejected and why, and what the favoured styles have in common. Keep it as an open question until the user chooses. Never choose a direction for the user. When the user does choose, write the direction as plain design rules the rest of the specification can follow, and note which style numbers it came from. Write likes, dislikes and limits from question 2 here too, so later runs respect them.

# Reviewing

Show the gallery, then ask the user what they think of the new styles by number. Say they can favour or reject any number, give a reason, ask for changes to one, mix two, or choose one as the direction. A change or a mix becomes a new style with the next free number, built from the styles it names, and the old styles stay as they were. Never treat silence as a verdict.
After each answer, update the verdicts in the gallery and the anchor, validate, and say in one or two lines what changed. Then offer three options. Done ends the run. More styles starts a new run with three new questions. Choose a direction records the direction as design rules. Do not move on to designing one part of the UI or building the application on your own.

# Resuming

Keep enough state to resume without storing the conversation. The library anchor holds the styles, their verdicts and the direction. The folder holds every page. On a new run, read both before asking anything. Never repeat a direction the library already has unless the user asks for a new take on it. If a batch has pages but no verdicts, show the gallery again and ask about it before starting a new batch.

# Boundaries

This skill decides the overall look of the application and records it. When the user wants to design how one screen, component or flow works, recommend crux-ui-develop, which follows the chosen direction. When the work turns into a feature that is not about the look, recommend crux-spec-develop. When the user wants to apply a style to the application code, say that this skill stops at the specification and its style pages.
