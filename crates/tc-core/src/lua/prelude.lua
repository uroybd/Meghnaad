-- The helper library every hook script runs with: a task, tags as a set, dates and durations with arithmetic, and the
-- few functions a hook may call. It is loaded before the user's script, in the same sandbox, and uses only what a
-- sandbox has (base, string, table, math) plus the `__host` functions the engine gives it (dates, printing).
--
-- The engine hands each hook a plain table for a task and takes a plain table back; `Task.wrap` and `Task.unwrap`
-- turn those into the objects a hook works with and back again. Nothing here can reach a file or the network.

local host = __host
__host = nil

------------------------------------------------------------------------------------------------------------------
-- Durations: a length of time, in seconds. `days(2)`, `hours(3) + minutes(30)`, `duration("3d")`.
------------------------------------------------------------------------------------------------------------------

local Duration = {}
Duration.__index = Duration

local function new_duration(secs)
  return setmetatable({ secs = secs }, Duration)
end

local function is_duration(v)
  return getmetatable(v) == Duration
end

-- Dates are defined below; a duration added to one hands it over.
local Date
local function is_date(v)
  return getmetatable(v) == Date
end

function Duration.__add(a, b)
  if is_duration(a) and is_duration(b) then
    return new_duration(a.secs + b.secs)
  elseif is_duration(a) and is_date(b) then
    return b + a
  end
  error("a duration can only be added to another duration or a date", 2)
end

function Duration.__sub(a, b)
  return new_duration(a.secs - b.secs)
end

function Duration.__unm(a)
  return new_duration(-a.secs)
end

function Duration.__mul(a, b)
  if is_duration(a) and type(b) == "number" then
    return new_duration(a.secs * b)
  elseif type(a) == "number" and is_duration(b) then
    return new_duration(a * b.secs)
  end
  error("a duration can only be multiplied by a number", 2)
end

function Duration.__div(a, b)
  if is_duration(a) and type(b) == "number" then
    return new_duration(a.secs / b)
  elseif is_duration(a) and is_duration(b) then
    return a.secs / b.secs
  end
  error("a duration can only be divided by a number or another duration", 2)
end

function Duration.__eq(a, b)
  return a.secs == b.secs
end

function Duration.__lt(a, b)
  return a.secs < b.secs
end

function Duration.__le(a, b)
  return a.secs <= b.secs
end

function Duration.__tostring(d)
  return host.format_duration(d.secs)
end

--- The length in the unit asked for: `d:in_days()`, `d:in_hours()`, `d:in_minutes()`, `d:in_seconds()`.
function Duration:in_seconds() return self.secs end
function Duration:in_minutes() return self.secs / 60 end
function Duration:in_hours() return self.secs / 3600 end
function Duration:in_days() return self.secs / 86400 end

function seconds(n) return new_duration(n) end
function minutes(n) return new_duration(n * 60) end
function hours(n) return new_duration(n * 3600) end
function days(n) return new_duration(n * 86400) end
function weeks(n) return new_duration(n * 604800) end

--- A duration written as Taskwarrior writes one: `duration("3d")`, `duration("2weeks")`, `duration("PT4H")`.
function duration(text)
  local secs = host.parse_duration(tostring(text))
  if secs == nil then
    error("not a duration: " .. tostring(text), 2)
  end
  return new_duration(secs)
end

------------------------------------------------------------------------------------------------------------------
-- Dates: a moment in time. Written and read as Taskwarrior does: `date("tomorrow")`, `date("2026-12-25")`,
-- `date("eom")`, `date("now+3d")`. `d + days(2)`, `d1 < d2`, `d2 - d1` (a duration), `d.year`, `d:format("Y-M-D")`.
------------------------------------------------------------------------------------------------------------------

Date = {}

local PARTS = { year = 1, month = 2, day = 3, hour = 4, min = 5, sec = 6, weekday = 7 }

Date.__index = function(d, key)
  local method = Date[key]
  if method ~= nil then
    return method
  end
  local i = PARTS[key]
  if i ~= nil then
    return host.parts(d.ts)[i]
  end
  return nil
end

local function new_date(ts)
  return setmetatable({ ts = ts }, Date)
end

function Date.__add(a, b)
  if is_date(a) then
    if is_duration(b) then return new_date(a.ts + b.secs) end
    if type(b) == "number" then return new_date(a.ts + b) end
  elseif is_date(b) then
    if is_duration(a) then return new_date(b.ts + a.secs) end
    if type(a) == "number" then return new_date(b.ts + a) end
  end
  error("a date can only be added to a duration (or a number of seconds)", 2)
end

function Date.__sub(a, b)
  if is_date(a) and is_date(b) then
    return new_duration(a.ts - b.ts)
  elseif is_date(a) and is_duration(b) then
    return new_date(a.ts - b.secs)
  elseif is_date(a) and type(b) == "number" then
    return new_date(a.ts - b)
  end
  error("a date can only have a duration or another date taken from it", 2)
end

function Date.__eq(a, b)
  return a.ts == b.ts
end

function Date.__lt(a, b)
  return a.ts < b.ts
end

function Date.__le(a, b)
  return a.ts <= b.ts
end

function Date.__tostring(d)
  return host.format(d.ts, "Y-M-D H:N")
end

--- Written with Taskwarrior's date-format letters: `Y` year, `M` month, `D` day, `H` hour, `N` minute, `S` second
--- (`m`, `d`, `h`, `n`, `s` without padding), `y` two-digit year, `A` and `a` weekday names, `B` and `b` month names.
function Date:format(pattern)
  return host.format(self.ts, pattern or "Y-M-D H:N")
end

--- The start of the day, week, month or year this date is in.
function Date:start_of(unit)
  local ts = host.start_of(self.ts, unit)
  if ts == nil then
    error("start_of takes 'day', 'week', 'month' or 'year', not " .. tostring(unit), 2)
  end
  return new_date(ts)
end

function Date:is_weekend()
  local w = host.parts(self.ts)[7]
  return w == 0 or w == 6
end

--- Seconds since 1970, as the date is stored.
function Date:epoch()
  return self.ts
end

function now() return new_date(host.now()) end
function today() return new_date(host.start_of(host.now(), "day")) end
function tomorrow() return today() + days(1) end
function yesterday() return today() - days(1) end

--- A date written as Taskwarrior writes one.
function date(text)
  if is_date(text) then
    return text
  end
  local ts = host.parse_date(tostring(text))
  if ts == nil then
    error("not a date: " .. tostring(text), 2)
  end
  return new_date(ts)
end

--- A date from its parts (local time): `date_of(2026, 12, 25, 8, 30)`.
function date_of(year, month, day, hour, minute, second)
  local ts = host.from_parts({ year, month, day, hour or 0, minute or 0, second or 0 })
  if ts == nil then
    error("there is no such date", 2)
  end
  return new_date(ts)
end

------------------------------------------------------------------------------------------------------------------
-- Sets: tags, and the tasks a task depends on. `tags:has("work")`, `tags:add("home")`, `tags:remove("old")`.
------------------------------------------------------------------------------------------------------------------

local Set = {}
Set.__index = Set

local function new_set(list)
  local s = setmetatable({ items = {} }, Set)
  for _, v in ipairs(list or {}) do
    s.items[v] = true
  end
  return s
end

function Set:has(value)
  return self.items[value] == true
end

--- True if it has at least one of these.
function Set:has_any(...)
  for _, v in ipairs({ ... }) do
    if self.items[v] then return true end
  end
  return false
end

--- True if it has every one of these.
function Set:has_all(...)
  for _, v in ipairs({ ... }) do
    if not self.items[v] then return false end
  end
  return true
end

function Set:add(...)
  for _, v in ipairs({ ... }) do
    self.items[v] = true
  end
  return self
end

function Set:remove(...)
  for _, v in ipairs({ ... }) do
    self.items[v] = nil
  end
  return self
end

--- The members, in order, as a list.
function Set:list()
  local out = {}
  for v in pairs(self.items) do
    out[#out + 1] = v
  end
  table.sort(out)
  return out
end

--- `for tag in task.tags:each() do ... end`
function Set:each()
  local list, i = self:list(), 0
  return function()
    i = i + 1
    return list[i]
  end
end

function Set:count()
  local n = 0
  for _ in pairs(self.items) do n = n + 1 end
  return n
end

function Set.__len(s)
  return s:count()
end

function Set.__tostring(s)
  return table.concat(s:list(), ", ")
end

local function is_set(v)
  return getmetatable(v) == Set
end

------------------------------------------------------------------------------------------------------------------
-- Tasks
------------------------------------------------------------------------------------------------------------------

local Task = {}

-- `end` and `until` are keywords in Lua: `task["end"]` works, and so do these.
local ALIAS = { finished = "end", expires = "until" }

local DATES = { "entry", "modified", "start", "end", "due", "wait", "scheduled", "until" }
local STRINGS = { "uuid", "status", "description", "project", "priority", "recur", "parent", "mask" }

local CORE = {
  tags = true, depends = true, annotations = true, blocked = true, blocking = true, imask = true,
}
for _, k in ipairs(DATES) do CORE[k] = true end
for _, k in ipairs(STRINGS) do CORE[k] = true end
for alias in pairs(ALIAS) do CORE[alias] = true end

Task.__index = function(t, key)
  local real = ALIAS[key]
  if real ~= nil then
    return rawget(t, real)
  end
  return Task[key]
end

Task.__newindex = function(t, key, value)
  rawset(t, ALIAS[key] or key, value)
end

--- Is the task in this project or one below it? `task:in_project("Work")` is true for `Work` and `Work.Mail`.
function Task:in_project(name)
  local p = self.project
  if p == nil then return false end
  return p == name or p:sub(1, #name + 1) == name .. "."
end

function Task:is_pending() return self.status == "pending" end
function Task:is_completed() return self.status == "completed" end
function Task:is_deleted() return self.status == "deleted" end
function Task:is_recurring() return self.status == "recurring" end

--- Is the task waiting (pending, with a `wait` date in the future)?
function Task:is_waiting()
  local w = self.wait
  return self.status == "pending" and w ~= nil and w > now()
end

--- Is it overdue (pending, with a `due` date in the past)?
function Task:is_overdue()
  local d = self.due
  return self.status == "pending" and d ~= nil and d < now()
end

--- The task as a hook works with it, from the plain table the engine gives.
function Task.wrap(raw)
  local t = setmetatable({}, Task)
  for _, k in ipairs(STRINGS) do rawset(t, k, raw[k]) end
  rawset(t, "imask", raw.imask)
  rawset(t, "blocked", raw.blocked)
  rawset(t, "blocking", raw.blocking)
  rawset(t, "tags", new_set(raw.tags))
  rawset(t, "depends", new_set(raw.depends))
  local notes = {}
  for i, n in ipairs(raw.annotations or {}) do
    notes[i] = { entry = new_date(n.entry), text = n.text }
  end
  rawset(t, "annotations", notes)
  for _, k in ipairs(DATES) do
    if raw[k] ~= nil then rawset(t, k, new_date(raw[k])) end
  end
  for k, v in pairs(raw.uda or {}) do rawset(t, k, v) end
  return t
end

local function check_string(name, v)
  if v ~= nil and type(v) ~= "string" then
    error("task." .. name .. " has to be text, not " .. type(v), 3)
  end
  return v
end

local function strings_of(name, v)
  if v == nil then return {} end
  if is_set(v) then return v:list() end
  if type(v) ~= "table" then
    error("task." .. name .. " has to be a list of text, not " .. type(v), 3)
  end
  local out = {}
  for _, s in ipairs(v) do
    if type(s) ~= "string" then
      error("task." .. name .. " can only hold text, not " .. type(s), 3)
    end
    out[#out + 1] = s
  end
  return out
end

local function timestamp_of(name, v)
  if v == nil then return nil end
  if is_date(v) then return v.ts end
  if type(v) == "number" then return v end
  if type(v) == "string" then
    local ts = host.parse_date(v)
    if ts == nil then error("task." .. name .. ": not a date: " .. v, 3) end
    return ts
  end
  error("task." .. name .. " has to be a date, not " .. type(v), 3)
end

local function uda_text(name, v)
  local kind = type(v)
  if kind == "string" then return v end
  if kind == "number" then
    if math.type(v) == "integer" then return string.format("%d", v) end
    return tostring(v)
  end
  if kind == "boolean" then return tostring(v) end
  if is_date(v) then return string.format("%d", v.ts) end
  if is_duration(v) then return string.format("%d", v.secs) end
  error("task." .. name .. " can't be saved: " .. kind .. " is not a kind of value a task can hold", 3)
end

--- The plain table the engine takes back for a task that a hook worked on.
function Task.unwrap(t)
  if type(t) ~= "table" then
    error("a hook has to return the task (or nothing), not " .. type(t), 3)
  end
  local out = {}
  for _, k in ipairs(STRINGS) do out[k] = check_string(k, rawget(t, k)) end
  out.imask = rawget(t, "imask")
  out.blocked = rawget(t, "blocked")
  out.blocking = rawget(t, "blocking")
  if type(out.description) ~= "string" then
    error("task.description has to be text", 3)
  end
  out.tags = strings_of("tags", rawget(t, "tags"))
  out.depends = strings_of("depends", rawget(t, "depends"))
  local notes = {}
  for i, n in ipairs(rawget(t, "annotations") or {}) do
    notes[i] = { entry = timestamp_of("annotations", n.entry), text = n.text }
  end
  out.annotations = notes
  for _, k in ipairs(DATES) do out[k] = timestamp_of(k, rawget(t, k)) end
  local uda = {}
  for k, v in pairs(t) do
    if not CORE[k] and type(k) == "string" then uda[k] = uda_text(k, v) end
  end
  out.uda = uda
  return out
end

------------------------------------------------------------------------------------------------------------------
-- What a hook may call
------------------------------------------------------------------------------------------------------------------

local function text_of(...)
  local parts = {}
  for i = 1, select("#", ...) do parts[i] = tostring((select(i, ...))) end
  return table.concat(parts, " ")
end

--- Say something: shown under the command's result.
function say(...) host.say(text_of(...)) end
print = say

--- Say something as a warning.
function warn(...) host.warn(text_of(...)) end

--- Stop the command, with this as the reason.
function refuse(message) error(tostring(message), 0) end

------------------------------------------------------------------------------------------------------------------
-- The network and secrets, where the host has them. Declare the hosts a script calls at the top of it:
--   allow_hosts{"acme.atlassian.net"}
-- then `http.get(url, opts)`, `http.post(url, opts)` (also `put`, `patch`, `delete`, and `http.request(method, ...)`).
-- `opts`: `headers = { Name = "value" }`, `json = <table>` or `body = "text"`, `timeout = seconds`. The answer is
-- `{ status, ok, headers, body, json }`, or `nil, reason` if the request could not be made or answered (the network,
-- a timeout): `local res, err = http.get(url)`. `secret("name")` is a value kept apart from the script.
------------------------------------------------------------------------------------------------------------------

allow_hosts = host.allow_hosts

http = {}

function http.request(method, url, opts)
  if type(url) ~= "string" then
    error("http needs an address as text, not " .. type(url), 2)
  end
  return host.http(method, url, opts)
end

function http.get(url, opts) return http.request("GET", url, opts) end
function http.post(url, opts) return http.request("POST", url, opts) end
function http.put(url, opts) return http.request("PUT", url, opts) end
function http.patch(url, opts) return http.request("PATCH", url, opts) end
function http.delete(url, opts) return http.request("DELETE", url, opts) end

--- Text made safe to put in an address: `"/search?q=" .. http.urlencode(task.description)`.
function http.urlencode(text)
  return (tostring(text):gsub("[^%w%-_%.~]", function(c)
    return string.format("%%%02X", string.byte(c))
  end))
end

secret = host.secret

------------------------------------------------------------------------------------------------------------------
-- What the engine calls
------------------------------------------------------------------------------------------------------------------

function __hook_launch(command)
  local f = rawget(_G, "on_launch")
  if type(f) == "function" then f(command) end
end

function __hook_add(raw)
  local f = rawget(_G, "on_add")
  if type(f) ~= "function" then return raw end
  local task = Task.wrap(raw)
  local result = f(task)
  if result ~= nil then task = result end
  return Task.unwrap(task)
end

function __hook_modify(old_raw, new_raw)
  local f = rawget(_G, "on_modify")
  if type(f) ~= "function" then return new_raw end
  local task = Task.wrap(new_raw)
  local result = f(Task.wrap(old_raw), task)
  if result ~= nil then task = result end
  return Task.unwrap(task)
end

function __hook_exit(raws)
  local f = rawget(_G, "on_exit")
  if type(f) ~= "function" then return end
  local list = {}
  for i, raw in ipairs(raws) do list[i] = Task.wrap(raw) end
  f(list)
end
