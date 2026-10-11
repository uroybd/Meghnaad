/** What the Hooks editor offers to start from: every hook, the useful ones switched on. */
export const HOOKS_EXAMPLE = `-- Hooks run on the server each time you run a command in this app. Define any of the four, or none.
-- (Your desktop's own hooks in ~/.task/hooks are separate: they never run here, and these never run there.)

-- Before a command runs. refuse("why") stops it.
-- function on_launch(command)
--   if command:find("purge", 1, true) then refuse("No purging from the web.") end
-- end

-- For each new task, before it is saved. Change the task you are given.
function on_add(task)
  if task:in_project("Work") then
    task.tags:add("office")
    say("Tagged +office.")
  end
end

-- For each task a command changes: \`old\` is how it was, \`new\` how it will be. Change \`new\`.
function on_modify(old, new)
  if old.status == "pending" and new.status == "completed" and old.blocked then
    refuse("Finish what it depends on first.")
  end
end

-- After the command, with the tasks it changed. It can only talk.
-- function on_exit(changed)
--   say(#changed .. " task(s) changed.")
-- end
`;
