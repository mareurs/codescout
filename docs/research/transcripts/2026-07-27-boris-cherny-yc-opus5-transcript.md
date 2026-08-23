# Boris Cherny: We Cut 80% of Claude Code's Prompt (Y Combinator, 35:51, 2026-07-27)

Source: https://www.youtube.com/watch?v=qyPCVqFUyDo
Captions: YouTube auto-generated (no human-authored subtitle track exists)

[00:03] [music]

[00:07] >> All right, Boris. We're so excited to have you here, the creator of Claude code.

[00:14] >> Thank you.

[00:17] >> [applause and cheering]

[00:19] >> It's great to be here.

[00:21] >> Fresh off the press. You guys just shipped Opus 5 yesterday.

[00:26] >> Yes.

[00:27] >> [applause and cheering]

[00:28] >> And it seems that model performance keeps accelerating. You guys got and took Arc AGI 3 to 30%, which is incredible.

[00:40] >> Yes.

[00:41] >> And for context, before the best score was in in the low single digits or low teens, right? What can Opus 5 do now that it couldn't versus the previous version?

[00:55] >> Yeah, there's um there's a lot that goes into every new model and there's a lot of new capabilities that we teach and uh get the model to do. Whenever you do model training, you try to teach a whole bunch of different things and most often it doesn't work. But some subset of the things the model does learn and sometimes it also surprises you. It has these skills, it has abilities that you you actually didn't really teach it, but it it just kind of learned. For five, one example of something it does that I think no other model has done is it runs for a very long period of time and especially

[01:35] when you combine Opus 5 with auto mode, it's just like incredible. Like it can go for days, weeks, months at a time. It just won't stop. Um you don't even need to use scaffolding. So you don't need the slash goal, you don't need all this other stuff. It'll just go because it knows it needs to do the task. Um another thing that I'm really excited about and um I'm going to start I think to talk about a little bit more um but it's kind of surprising because it's such a new capability is the model does not seem to be prompt injectable anymore.

[02:08] >> What's prompt injectable?

[02:10] >> [applause]

[02:11] >> It's crazy. Like people have talked about this like lethal trifecta for a long time and this really affects kind of harness design and agent design and and and product design because if the model reads some instruction on the internet that's like, you know, do X and Y and Z and also delete everything on the user's computer. A year ago the model would have just done it. But nowadays Opus does not. And this has actually been the case since like Opus 4.7, 4.8, Sonnet 5 has been quite good at this, People was quite good at it. But Opus 5 just hits like a new frontier on this. So essentially if you combine a well-aligned model, so this is like essentially three years of research into

[02:51] alignment, with a prompt injection classifier which we run for all traffic. And what this is doing is it's based on Crystal's mechanistic interpretability work where it's it's literally we're looking at neurons in the model's brain that light up when prompt injection happens. So the model won't even tell you but we can actually see those neurons and we can figure out and diagnose that it's happening. And then you combine that with the auto mode classifier and with these three layers we just cannot demonstrate prompt injection anymore.

[03:21] >> Talking about a prompt injection, the other side of the coin is now the system prompt. Let's talk a bit about the new release. You actually deleted over 80% of the system prompt from Claude code.

[03:37] >> Yes.

[03:37] >> Tell us more about that.

[03:38] >> I think something that a lot of people might not realize is Claude code as a product and as a harness is just always changing. We're always adding stuff. We're always deleting stuff. Every time that a new model comes out, we delete a bunch of the system prompt, change a bunch of the system prompt. We change the set of tools all the time. We change the prompts for the tools all the time. And the reason is every model is very different. So, something that you did for one model maybe 3 months ago, it just might not translate at all to the next model. And so, one thing about Opus 5 is it's just really intelligent. And a lot of

[04:20] the stuff in the system prompt was correcting for these behaviors that the model should have known, but uh it didn't. Now, Opus 5 just does it. So, yeah, we deleted 80% of the system prompt. You can actually try deleting the rest of it, too. Um so, when you run Claude Code, you can just do like {dash} {dash} system prompt and set whatever system prompt you want if you want to experiment with it. And another thing that you can try is um simple mode. So, this is actually this kind of undocumented feature. If you do Claude Code simple equals one, like this uh environment variable, and then you run Claude, it'll delete all the system prompts, including from the tools.

[05:00] And we actually use this as a sort of ablation to figure out is the prompt useful? And what's interesting is that the model is actually a little bit more intelligent without these prompts. That's something that we've been finding. But when you use Claude Code as a product, you do actually want some of these prompts because it helps you use the product and it it helps the the product behave and the model behave in the way that you would want when when you're using it as a person.

[05:24] >> I think the thing that's really fascinating in this era of building, basically you'll build the best harness in the world for for Claude, and that's Claude Code. From what I'm hearing, you for every model released, you basically delete all of the code base, delete all of the prompt, and start from scratch every time. That in the old world would have been not something startups would have done for the product cuz I press delete every 6 months for everything.

[05:52] >> That's right. That's right. We so to be fair, we don't delete the entire code base, but we do delete a lot. So every time there's a new model, we try we call it in a research you call this a ablation. And so what this means is you delete the entire system prompt and then you bring it back line by line to figure out what is the impact of each individual line. Um it's sort of like a eval and you can kind of like evaluate it and ablation essentially is a eval where you delete things to figure out the impact. And yeah, like we do the same thing for tools. Like we unship tools all the time. We you know, delete code in the harness all the time. If you look at actually the code that's in the Claude code harness today, almost all of it is about safety and permissions and static analysis and there's a bunch of UI code

[06:33] and we've actually unshipped a lot of the other code already.

[06:37] >> Do you think this way of building a agentic product and harness and basically doing ablations every time with a there's a new model release, should everyone in this room that's building AI products basically do that? Be comfortable and brave to press delete.

[06:55] >> 100%. Yeah, and and for people that aren't building agentic products, but you're using Claude code, every 6 months delete your Claude MD. Delete your skills. Delete your hooks. See what the model does and it might surprise you. And actually for Opus 5, this is something we really do recommend is just try deleting all of these things because the model might really just not need all those instructions that you needed for past models.

[07:20] >> Let's talk a bit about how then you build this new prompt. When there's a new model release, like for everyone in the room, everyone will want to try Opus 5 and they're going to press delete on their system prompt. How do they go about building rebuilding their system prompt? How do you set up your environment?

[07:39] >> So you do you do it kind of piece by piece. So, the first step is you delete. The next step is you use it. And you don't want to guess what's the instruction that the model needs because you might not predict it correctly. The thing that you want to do is you want to run it. And if it's like a customer agent like product that you're building, you want to kind of run the product. Uh you want to see where it fails with the model. You want to see what it does well. If you're using quad code, you want to see where it does well with your code base or maybe where it stumbles over, you know, the architecture or stumbles over something else. And only when you see it repeatedly stumble on the same thing, that's when you add it back.

[08:20] But you don't want to do it too early because remember like the model is going to read this instruction every single time you use it. So, you really want to make sure that the model needs this instruction. I I think this is sort of the crazy thing about building on models. It's just so different than all the engineering that I've ever done. Like in the past when you built on systems, you built these like big beautiful systems and you really think about the system design up front. You have like a big suite of unit tests. You think about everything and you know, like a re-architecture is a big project. It sometimes it takes months. I've worked on re-architecture products at, you know, big companies that take years. And the model is not like that. It's um the way to think about it is almost like

[09:00] a like a living creature, like it's something more organic. It's a thing where every model generation, it behaves differently. It has a slightly different personality. And you have to take the time to get to know it and then adjust the harness based on that. And I I think it's just very much like an empirical and kind of scientific thing. You have to take a very scientific mindset to it where you try something, you see the result, and then you iterate based on that.

[09:25] >> If you're building in this world right now, what then becomes uh stable? Are evals something that you keep from the previous models and keep using them in each new model release?

[09:36] >> Um we do until we max out the eval.

[09:40] >> So, that's sort of the tip for everyone. So, code and system prompt, you have if you want to build at the bleeding edge and have the most capability for models, you got to delete those, but evals are constant and keep appending to them, basically.

[09:53] >> Yeah, you keep you keep appending. What happens is um you know, I actually wouldn't even go this far, to be honest. I think evals, they outlive the harness a little bit, but not by that much. Like an eval might live for maybe one, two, three model generations, but nowadays the you know, we're on the exponential. The model is improving so quickly, very often we just saturate the eval, and then we have to throw it away, and we have to come up with a new eval. And this is just part of the process. And again, it's about being empirical. You have to use the product, you have to use the model, you have to see where it struggles, and then based on that, that's the eval set that you should build.

[10:30] >> I think one one term I heard you describe how to build the best agentic products on top of a Claude is this concept of a unhobbling Claude. And tell us more more about what that means.

[10:47] >> Yeah, so hobbling is this idea in a research that the model is doing something and you're just getting in the way. There there's this kind of like way of thinking about it that I really like. It's very useful when you're building product. And um it it it's called product overhang. And then the idea is the model is able to do all sorts of things with today's models, not a future model, but today's model, that we have not yet realized. And there are so many capabilities the model has like this that people are not aware of. And this is like the ability to, you know, like maybe use a particular tool,

[11:29] use a particular language, solve a particular kind of problem, do things a particular kind of way that we thought was kind of beyond the model's capability. And um there's this overhang. Because the model can do this at every given model generation, but there is often not a product that lets the model do this and lets it express this kind of ability to do this. And on the flip side, often what happens is the product gets in the way. And this getting in the way, we call this hobbling. And then not not eliciting the correct behavior from the model, we call this product overhang. So, it's kind of like two sides of the same thing. What One example of this was the original Claude Code.

[12:10] When I first started working on it, this was um you know, like a year and a half, 2 years ago, something like that. This was like Sonnet 3.5. At the time, that was an incredible coding model. That was like the best coding model that exists. Nowadays, it's, you know, a pretty terrible coding model by modern standards. But I think that was like the first great coding model that that we built as Anthropic. And at the time, if if you looked at the coding products of the time, what were what were they doing? They were doing like single-line auto complete. They were doing sometimes multi-line auto complete. That was sort of a new idea. Um they were they were doing chat. So, you can talk to the agent, but it wasn't uh right access. You could only read. You could ask about the code base.

[12:51] And so, the the feeling was that there wasn't really a product that was fully eliciting the model's capability to write entire functions at a time, entire files at a time. At the time, it wasn't entire features. We weren't there yet, but probably entire files. That's that was the level of capability at the time. And so, the idea with Claude Code was, all right, we think the model can probably do this. What if we get rid of all the scaffolding and just give the model the simplest possible harness, so it can write an entire file at a time and build an entire feature? Um and that was that was kind of it. Like, was the product overhang of the time. The model was capable of doing something

[13:31] and everything was just kind of getting in the way. I I think that nowadays, with modern models, there's so much product overhang that I have I'm not seeing startups capture. And I think there's people thinking about these problems, but there's just a huge amount of amount of opportunity to elicit these behaviors from the model that are just like amazing and interesting and and commercially valuable.

[13:54] >> I think this is such a special insight for everyone here in the room. Basically, all of you could create the next Claude code if you figure out how to un-hobble the models because that's effectively the birth story of Claude code. You un-hobble Sonic 3.5 because all the previous iterations were still getting the model very rigid in in IDEs. And Claude code was one of the first instances that gave it just a full terminal access.

[14:25] >> Yes.

[14:25] >> And that then created this amazing product just that keeps going. So, let's talk about um what are some areas and how should future founders here think about un-hobbling Claude and fixing this product overhang?

[14:47] >> So, there's a couple of things that I would think about. One is you should give the model slightly harder tasks than what you think it can do. I think a a really common mistake that I see is people are using Claude code, they're using Claude, and they they just give it like way over we specific instructions. They're like, "I want you to do this, but I want you to do it in this way, this way, this way. You must do like one, then two, then three, then four." And for modern models, that's actually really not the way to do it. You want to go a little bit higher level. You want to describe the task, you want to describe the guardrails, you want to describe like the exit criteria, and then just go with the model cook. And come back in a little bit.

[15:29] And I think it'll it'll surprise you. Like and again, like this is just not something that would have worked 6 months ago, but it does work today.

[15:35] >> Can you give some examples of these challenging tasks or capabilities that people should explore that it can do now that it couldn't 6 months ago?

[15:44] >> Yeah. So, okay, one example is the model can now rewrite essentially any code base from one language to a different language. It's just sort of crazy. Like it's this work that would have taken just like a very long time as an engineer, and now the model's like quite fast at it. So, so one example of this is um Cloud Code is built on the Bun JavaScript runtime. It's a open source JavaScript runtime. Um it's an alternative to Node.js. It's kind of a faster node. Bun was written in Zig. Zig is a systems programming language. It's It's kind of like C. It's It's very low level. One of the problems with C with a with Zig is you have to manually

[16:24] manage memory. And so it's quite easy to run into situations where there's like memory leaks and, you know, other memory management issues. And so, one thing that the Bun team was doing is they were having Claude fuzz the code base and try to simulate and trigger memory leaks, and they were doing this for, you know, for a long period of time. They were able to find a lot of memory leaks. It was sort of like a case at a time. And that was kind of the capability of the model at the time was doing this fuzzing. And then at some point, Jared on the team was like, "Okay, let's just like rewrite it. Maybe the model can do this." And I I think this is like one of these test problems that he kind of threw at the model with every new model generation.

[17:04] And starting with Fable, the model started to be able to do it. And so I think Opus 5 could do it as well. And so what he did was essentially he defined a test suite. The nice thing about Bun is it's very, very well tested. There's a big test suite in Bun, there's a big test suite in Node.js. So, it's easy to know if you did the right thing. And he had the model rewrite it from Zig to Rust. It was one prompt. It was a dynamic workflow. And a dynamic workflows are a feature in Claude Code that essentially let you orchestrate, you know, dozens, hundreds, thousands of agents to do work productively. And it ran for 11 days, and it rewrote the entire code base.

[17:44] >> And this was one shot?

[17:45] >> It was one shot with It was No, it wasn't one shot, but it There was steering. There was steering. Um but previous models just couldn't do this, even even with the steering. It just wouldn't have been possible.

[17:55] >> 11 days? Oh my god. This would have taken in the past even with the best engineers, multiple months, years?

[18:02] >> Over Definitely over a year.

[18:04] >> Yeah.

[18:04] >> Yeah, over a year. This is like over 100,000 Like JavaScript runtimes really complicated. There's There's a lot of stuff in there. Um and yeah, it like it works. This is in production now. This is what Claude Code uses now when when you're running it. So, this is kind of one example. I I would give a second example also of product overhang. And so, this is like a practical use case where like there's a problem you're solving. It's like a business problem, an engineering problem, a product problem. And you should just keep throwing the latest model at it to see if it'll just do it. Cuz even if a previous model didn't, the new one might. I think the second way to think about it is experiment. And just give yourself like freedom to play with the model and do creative things.

[18:44] Often it'll surprise you. So, something that's actually been really popular at internally that's been kind of viral within Anthropic the last couple weeks is someone figured out that you can give Opus 5 OpenCV.

[18:55] >> Oh.

[18:56] >> And you can have it draw. And so, something you can do is you can ask Opus like, "Hey, use Open OpenCV to like draw this image." And it's actually quite good. It can do like portraits, it can draw like animals, it can do like landscapes. And we didn't train the model to draw. Like it it's just like the solicitation gap. Like if you ask it to do it the right way, it can just do it. And we discovered this kind of accidentally just by playing around and trying creative things that didn't have direct commercial applications. But it's just kind of interesting. And my hypothesis is there's probably dozens, hundreds of opportunities like this with the models of today that no one has yet realized.

[19:32] >> And the big area of research for this is basically model elicitation, right? Becoming really good at figure out all these capabilities and asking the model to do the right thing, right?

[19:44] >> Yes.

[19:45] >> How do people get better at that? And effectively, how do people get better at prompt engineering? Do people still need to do a lot of prompt engineering? Or is that changing as well? Tell us about where this is going.

[19:57] >> Yeah, I remember like a year ago one of the most popular job openings was prompt engineer. And then it kind of changed and then I think it became like context engineer. So there's these kind of waves of it. I think I think these will kind of like come and go. I think the skill nowadays is less about prompt engineering and more about figuring out how do you give Claude a hard task that seems a little bit too hard. And then how do you make it possible for Claude to verify its work along the way? And the verification I think is probably the single most important thing that people do not get right or actually. Um one example of this is people were you

[20:41] know, we have this desktop app for Claude. And it's built using electron. We've made it quite fast. So now it's like a pretty awesome experience. Six months ago it was like sluggish and it wasn't very reliable. Now it's pretty awesome. And you know, it's the thing that most of the team uses. As an experiment though, I wanted to see like what would it feel like if it was native? And so what I did is I I started a Claude tag session. And Claude tag is just you know, it's a it's a new product we have. It's just quad running in Slack. My first question was, "Hey Slack, do you have access to a Mac OS runner on GitHub?" And uh it said no. And then I I hooked up a runner, so it was able to start a Mac virtual machine uh using using GitHub. And then um my second question is uh I

[21:21] created this like empty code base that was uh quad desktop app rewritten in Swift. And I asked, "Can you access this code base?" It said no. And then I gave it access and I was like, "Okay, great. Now I have access." And then I was like, "Okay, now I want What I want you to do is I want you to rewrite the Electron app in Swift. I want you to run the Electron app in the Mac virtual machine, screenshot it, and then look pixel by pixel, compare it to the Swift version, don't stop until you're done."

[21:54] >> And that was your prompt, basically.

[21:56] >> That was my prompt.

[21:57] >> And how long did this take to run?

[21:59] >> It's still running.

[22:02] >> When did you start it?

[22:03] >> [laughter]

[22:04] >> It's been uh it's been a little over 2 weeks, so it's like 14 days, 15 days.

[22:09] >> Yeah, so I don't know if anyone in the audience has gotten Claude to run a a a task for more than 2 weeks. I don't know. If raise your hand, anyone in the audience.

[22:22] >> Oh.

[22:23] >> All right. Some some.

[22:25] >> This is This is like one of these um this is about hallucination. So, it's So, this is really one of those examples where the model can do it today. You just have to let it do it. And you don't need the fancy stuff. You don't need slash goal, you don't need slash loop. These help, but really all you need is give the model the task, give it a way to verify the output of its work so it doesn't get stuck, and it will just go. And actually in this case, Claude also decided to live blog it. So, what it did is it created a Slack channel internally and it started just posting screenshots every few minutes of its progress.

[22:57] >> Wow. So, the prompt sounded so simple. I mean, everyone here could do it. And um I guess what is separating the people here that can become the top 1% Clocko users? How do How do How do people learn to use Clocko like Boris?

[23:17] >> Maybe like don't listen to the LinkedIn influencers.

[23:21] >> Don't listen to Don't read Twitter.

[23:24] >> [cheering] [applause]

[23:28] >> This is the thing about the model is uh I think everyone's looking for like the one weird trick to do it. The There's just like that doesn't exist. There's nothing like that. The The way the model works is you have to approach it empirically. You have to give it a task that's too hard. You have to give it the tools to verify the work like you would yourself, like you would if you were doing the task. You have to see where it struggles and then uh you have to like fix that either with better prompting or with a skill or if the model's missing context like give it a MCP so it can pull in the context that uh that it needs. Uh that's kind of it.

[23:59] >> It sounds very simple.

[24:01] >> [laughter]

[24:01] >> I think people tend to overthink it a little bit. I think people tend to overengineer. Cuz I think in a lot of ways like when we built systems in the past, that's the way you had to do it. So when I look at engineers that have been, you know, coding for a long for a long time, you know, like for for years or for decades, this is a really really common failure mode is trying to over specify and it's trying to be overly specific and then, you know, get the model to do the to do the task exactly the way that you would have done it. And that that's just not the way the model works. But I think a lot of people are kind of unlearning this and it's a journey to to unlearn it. And um it's a journey to kind of figure out how how do you treat this thing like you would a coworker. I think that's the level of intelligence that it's at now.

[24:42] >> And as part of this, let's go deeper into this task that's still running. Two weeks since you launched it a go two weeks ago. How many agents did it spawn?

[24:50] >> You know, I'm not sure. I I can ask Bod and then I I can get back to you. I would I would guess thousands, tens of thousands.

[24:57] >> Thousands. Has anyone in the audience had a uh prompt to to any of the models that run that spawn more than a thousand agents? No? I think this is another of the tips, like the best Claude users are able to spawn tasks that are really providing you a lot of leverage, like thousands of agents.

[25:17] >> Yes.

[25:18] >> How do you do that?

[25:19] >> There There's a few different ways to do it. Um the easiest way is dynamic workflows. To use dynamic workflows, it's a fairly new feature in Claude code. And all you have to say is use a workflow. That's it. And then Claude will just trigger the dynamic workflow. What a dynamic workflow is is essentially we have the we have the Bun runtime. We use Bun as a sandbox, and we start a virtual machine within Bun. And we let Claude start a lot of agents and orchestrate them. And it it doesn't just do one agent, it doesn't just do like 10 parallel agents. What it might do is um let's say a task is like rewrite the code base, or do really

[25:59] in-depth data analysis over some really complicated data, or maybe like build a very complex feature that takes multiple stages, and maybe dozens of pull requests. And so what it's going to do is it's going to start a bunch of agents to do kind of like the first pass. Based on that, it might do a second step where it has another set of agents that verify the work, or that summarize the work. Then it might do like a third stage, where it'll fan out again. So it'll kind of productively orchestrate a bunch of different agents. So my background is functional programming. And so the way that we designed this is is essentially an algebra for agents. So there's a way to run agents in sequence. There's a way to run agents in

[26:40] parallel. And Claude has different tools in order to orchestrate these agents inside of the sandbox, to use tokens efficiently, to do really, really complex work. It's kind of cool and something that just hasn't really been written about a lot. Like this is actually like a new form of test time compute. Like when we talk about the scaling laws and kind of we talk about the model getting more intelligent over time, historically, it's been a function of the size of the neural net, the amount of training data, and the number of flops that you put in to the training. And then recently, we also added test time compute. So this is essentially a fancy way a researcher way of saying how many tokens does it generate.

[27:21] And now dynamic workflows are essentially a new way to orchestrate test time compute. And it's a new way to kind of really, really ramp up the amount of test time compute that you use to do a really hard task. So this all very long way to say this is one way to launch thousands of agents in a way that is productive and efficient. A second way to do it is loops and routines. Loop is essentially a cron job that's running locally for Quad. Routine is the same thing, but it's running the Quad in the cloud. So you can close your laptop. And this is like slightly different because for a dynamic workflow, it's one task and you break it up into chunks. For loops and routines, it's one task

[28:01] that is repetitive, that doesn't share context, but it might share memory. And you kind of do this like over and over. You can do it like maybe every hour, every 5 minutes, every day. And so the thing that we've started doing is um we actually have Quad maintaining itself now. And the way we do this is we have a Slack channel where we just had Quad start a bunch of different routines to maintain its own code base. And we actually do this for the CLI, for the iOS app, for the Android app, uh for the desktop app. And you for example, one routine is clean up dead code. This is a single prompt. It's like one sentence. Quad runs this every day. It'll look for dead code across all the code bases using static and dynamic

[28:42] analysis. We didn't prompt that. It just kind of figured it out. And it'll put up a pull request every day to the weak the dead code. Another example is shipping experiments that should go out. Um so the experiment's already out to 100%. It'll delete it from the code base and it will just ship it. Another one is writing tests for areas of the code base that need test coverage. Another one is deleting tests that don't need to be there cuz you know, they were kind of useless tests added by older models or added by people at some point. One that one that I really love is this um I forgot what we called it. I think we called it abstraction police. And the idea is there are often in a big code base, there's kind of the same abstraction and it appears multiple

[29:23] times and if you kind of squint it actually maybe should just be the same abstraction, but kind of over time for whatever reason you rebuilt it multiple ways in different parts of the code base. So Quad kind of goes out every day across all our code bases, it finds these nearly duplicated abstractions and it unifies them. And so now we have every day maybe 20 or 30 of these routines. It's running across all of our code bases and it's not totally there yet, but we're on the path to fully automating the maintenance of our apps by doing this. And this is again hundreds of agents running every day, sometimes thousands of agents every day. It's doing the work of you know, dozens or hundreds of engineers. This is kind of what it used

[30:04] to take to do this kind of work. And this means that engineers can just like do the thing they actually want to do, which is ship new product and talk to users and do stuff that's actually fun.

[30:15] >> I guess nice conclusion from this, which you have mentioned in the past that basically coding is solved, right? You have mentioned this. Um I'm curious now that effectively everyone can write software, what separates the exceptional builders from the rest? What what are the qualities now that everyone can ship code?

[30:38] >> I I would give like one caveat. So coding is solved for the kind of coding that I do. It's not solved for everyone. You know, there's still code bases that are like super deep systems code bases where quad still struggles. There's distributed systems where quad still struggles. There's really kind of in the weeds UI verification, like something is off by pixel or something. Quad is still not perfect at this. Like Opus 5 was a big leap in vision and computer use, but it's still not perfect. Um but I I'm actually curious for people here, maybe raise your hand if 100% of your code is written using agents. You don't write any code by hand anymore. It's pretty good. Okay, how about more

[31:18] than 50%? Slightly less hands, maybe about the same. Yeah. So I think it's like it's getting there. So it's kind of getting to this to you know, to being solved for more and more kinds of code. And that's kind of cool. When I think about the people that are the best at using quad, I think there's a certain mindset that you can bring that's really effective. And it's really about being empirical. So forget all of the things that you learned about past models. Forget everything that you've learned about computer science theory in class. Look at the model, try to do a task, see where it

[31:58] struggles, and then based on that adjust. So it's just like very much become it's not a theoretical science, it's become an empirical science. So I think people that are really good at this, that are really good at kind of forgetting their priors, letting go of you know, this like maybe idea that didn't work before and just being open to trying it again. This is the kind of skill that's just very, very successful now.

[32:20] >> Now my last question is given everything that we talked about, if there's someone here that's studying CS, and you you learned to program before this era of AI agent coding, what should students still learn the hard way, like the old way.

[32:39] >> So for me, I learned computer science practically. I learned it by teaching myself to code in order to solve problems. Whenever I was doing this, I was doing it to solve a particular problem that I had. So I actually first learned to code on a TI-83 calculators. Um this is back in middle school and um I ended up actually writing a guide on the internet for programming TI-83 calculators. It's still up on the internet somewhere. Um and it was uh it was basic. That that was my first language. And I I learned how to program on a calculator so I could just like get better at my math tests by uh by cheating on the test.

[33:21] >> [applause]

[33:25] >> So it it it was about something practical, you know, that like to me as a middle schooler that was kind of like the most practical thing I could think of. And I ended up getting good grades and then I got this little serial cable to give the you know, the programs to my classmates and they got really good grades. And then the math got a little bit harder. Um it wasn't something that I could solve in basic anymore. So I kind of went from this like, you know, like maybe algebra solver that was written in basic and I had to solve harder problems. And um you know, like once we got into calculus, I had to learn assembly so I can write a better solver so I could cheat better on the test now that it was calculus. And so for me, programming has always been very practical and I think this is always my advice for people in school is

[34:06] learn not just the computer science. This is like intellectually fascinating and it's really really interesting to know, but learn how to apply it. And often this is about building startups. It's about building products. It's about developing your own design sense, developing your business sense, learning how to how to do data science, learning how to talk to users. There are all these other skills and when you combine it with computer science and engineering, that's where it becomes really really valuable. So those are the hard skills that I would still be doing by hand.

[34:36] >> So, if I'm hearing and summarizing, start with making something you want first for yourself, and then level up and make something people want.

[34:45] >> Yes.

[34:47] >> And we just have one last special announcement for us. You want to one one last thing?

[34:53] >> Yeah, so um for everyone here today, uh you are getting Max 20X.

[35:03] >> [cheering] [cheering] [laughter]

[35:15] >> Pretty good.

[35:16] >> [applause]

[35:17] >> So, look for look for a code in your email. And uh I can't wait to see you what you build.

[35:22] >> We'll send an email.

[35:26] >> [applause] [cheering] [applause]

[35:31] >> So, I'm curious. Someone in this room should be building something that runs hopefully multiple months and thousands of agents now that you have the account to do it. And with that, thank you so much, Forrest.

[35:42] >> Thank you.

[35:43] >> Thank you.

[35:43] >> [cheering] [applause]
