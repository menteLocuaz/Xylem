local M = {}

local uv = vim.uv
local job_id = nil
local stdout_buffer = ""
M.notifications = {}
local notification_id = 0
local attached_buffers = {}
M.hl_ns = vim.api.nvim_create_namespace("xylem")

local function generate_id()
    notification_id = notification_id + 1
    return "sync_" .. notification_id
end

function M.start()
    if job_id then
        vim.notify("[xylem] Already running", vim.log.levels.WARN)
        return
    end

    local binary_path = vim.fn.stdpath("data") .. "/xylem/target/release/xylem"

    if not vim.fn.executable(binary_path) then
        binary_path = "./target/release/xylem"
    end

    job_id = vim.fn.jobstart({
        binary_path,
        "--rpc",
    }, {
        rpc = false,
        stdout_buffered = false,
        stderr = "pipe",

        on_stdout = function(_, data)
            if data then
                stdout_buffer = stdout_buffer .. table.concat(data, "\n")
                while true do
                    local header_end = stdout_buffer:find("\r\n\r\n")
                    if not header_end then break end

                    local header = stdout_buffer:sub(1, header_end - 1)
                    local content_length = header:match("Content%-Length: (%d+)")

                    if not content_length then
                        stdout_buffer = stdout_buffer:sub(header_end + 4)
                    else
                        content_length = tonumber(content_length)
                        if #stdout_buffer < header_end + 3 + content_length then
                            break
                        end

                        local body = stdout_buffer:sub(header_end + 4, header_end + 3 + content_length)
                        stdout_buffer = stdout_buffer:sub(header_end + 4 + content_length)

                        local ok, decoded = pcall(vim.json.decode, body)
                        if ok then
                            M.handle_message(decoded)
                        else
                            vim.notify("[xylem] Failed to decode JSON: " .. body, vim.log.levels.ERROR)
                        end
                    end
                end
            end
        end,

        on_stderr = function(_, data)
            if data then
                for _, line in ipairs(data) do
                    if line ~= "" then
                        vim.notify("[xylem] ERROR: " .. line, vim.log.levels.ERROR)
                    end
                end
            end
        end,

        on_exit = function(_, code)
            job_id = nil
            if code ~= 0 then
                vim.notify("[xylem] Process exited with code " .. code, vim.log.levels.ERROR)
            end
        end,
    })

    if job_id > 0 then
        vim.notify("[xylem] Started successfully", vim.log.levels.INFO)
        M.setup_autocmds()
    else
        vim.notify("[xylem] Failed to start", vim.log.levels.ERROR)
    end
end

function M.handle_message(msg)
    local method = msg.method
    local id = msg.id
    local params = msg.params

    if method == "xylem.sync.result" and id and M.notifications[id] then
        local cb = M.notifications[id].callback
        if cb then
            cb(params)
        end
        M.notifications[id] = nil
    elseif method == "xylem.sync.progress" then
        local cb = M.notifications[id] and M.notifications[id].on_progress
        if cb then
            cb(params)
        end
    elseif method == "xylem.sync.complete" then
        local cb = M.notifications[id] and M.notifications[id].on_complete
        if cb then
            cb(params)
        end
        M.notifications[id] = nil
    elseif method == "xylem.highlights" then
        M.apply_highlights(params.buffer_id, params.highlights)
    elseif method == "xylem.highlights.delta" then
        M.apply_highlight_delta(params)
    end
end

function M.send_message(msg)
    if not job_id or job_id <= 0 then
        return
    end

    local encoded = vim.json.encode(msg)
    local header = string.format("Content-Length: %d\r\n\r\n", #encoded)
    vim.fn.chansend(job_id, header .. encoded)
end

function M.sync(opts)
    opts = opts or {}
    local lang = opts.lang or "lua"
    local callback = opts.callback

    local id = generate_id()
    M.notifications[id] = { callback = callback }

    M.send_message({
        method = "xylem.sync",
        id = id,
        params = { lang = lang },
    })
end

function M.sync_all(opts)
    opts = opts or {}
    local on_progress = opts.on_progress
    local on_complete = opts.on_complete

    local id = generate_id()
    M.notifications[id] = { on_progress = on_progress, on_complete = on_complete }

    M.send_message({
        method = "xylem.sync_all",
        id = id,
    })
end

function M.setup_autocmds()
    local group = vim.api.nvim_create_augroup("xylem", { clear = true })

    vim.api.nvim_create_autocmd("BufEnter", {
        group = group,
        pattern = "*.lua",
        callback = function(args)
            M.attach_buffer(args.buf)
        end,
    })

    vim.api.nvim_create_autocmd("BufLeave", {
        group = group,
        pattern = "*.lua",
        callback = function(args)
            M.detach_buffer(args.buf)
        end,
    })
end

function M.attach_buffer(buf_id)
    M.send_message({
        method = "xylem.attach",
        params = { buffer_id = buf_id },
    })

    if not attached_buffers[buf_id] then
        attached_buffers[buf_id] = true
        vim.api.nvim_buf_attach(buf_id, false, {
            on_bytes = function(_, buf, changedtick, start_row, start_col, old_end_row, old_end_col, new_end_row, new_end_col)
                local start_byte = vim.api.nvim_buf_get_offset(buf, start_row) + start_col
                local old_end_byte = vim.api.nvim_buf_get_offset(buf, old_end_row) + old_end_col

                local line_count = new_end_row - start_row + 1
                local lines = vim.api.nvim_buf_get_lines(buf, start_row, start_row + line_count, false)

                local new_text
                if line_count == 1 then
                    local line = lines[1]
                    local byte_len = #line
                    new_text = line:sub(start_col + 1, new_end_col)
                else
                    local parts = {}
                    for i, line in ipairs(lines) do
                        if i == 1 then
                            table.insert(parts, line:sub(start_col + 1))
                        elseif i == line_count then
                            table.insert(parts, line:sub(1, new_end_col))
                        else
                            table.insert(parts, line)
                        end
                    end
                    new_text = table.concat(parts, "\n")
                end

                M.send_message({
                    method = "xylem.change",
                    params = {
                        buffer_id = buf,
                        start_byte = start_byte,
                        old_end_byte = old_end_byte,
                        new_text = new_text,
                    },
                })
            end,
        })
    end
end

function M.detach_buffer(buf_id)
    M.send_message({
        method = "xylem.detach",
        params = { buffer_id = buf_id },
    })
    attached_buffers[buf_id] = nil
end

function M.apply_highlights(buf_id, highlights)
    if not vim.api.nvim_buf_is_loaded(buf_id) then
        return
    end

    vim.highlight.clear(buf_id, "xylem")
    for _, hl in ipairs(highlights) do
        local start_pos = M.byte_to_pos(buf_id, hl.start_byte)
        local end_pos = M.byte_to_pos(buf_id, hl.end_byte)
        vim.highlight.range(
            buf_id,
            "xylem",
            hl.hl_group,
            start_pos,
            end_pos,
            {}
        )
    end
end

function M.apply_highlight_delta(params)
    local buf = params.buffer_id
    if not vim.api.nvim_buf_is_loaded(buf) then
        return
    end

    for _, delta in ipairs(params.deltas) do
        vim.api.nvim_buf_clear_namespace(buf, M.hl_ns, delta.line, delta.line + 1)
        for _, cap in ipairs(delta.captures) do
            vim.api.nvim_buf_add_highlight(buf, M.hl_ns, cap.hl_group,
                delta.line, cap.start_col, cap.end_col)
        end
    end
end

function M.byte_to_pos(buf_id, byte)
    local text = vim.api.nvim_buf_get_lines(buf_id, 0, -1, false)
    local pos = { 0, 0 }
    local current_byte = 0

    for i, line in ipairs(text) do
        local line_bytes = #line + 1
        if current_byte + line_bytes > byte then
            pos = { i - 1, byte - current_byte }
            break
        end
        current_byte = current_byte + line_bytes
    end

    return pos
end

function M.stop()
    if job_id and job_id > 0 then
        vim.fn.jobstop(job_id)
        job_id = nil
        vim.notify("[xylem] Stopped", vim.log.levels.INFO)
    end
end

function M.is_running()
    return job_id ~= nil and job_id > 0
end

function M.get_status()
    return {
        running = M.is_running(),
        job_id = job_id,
    }
end

return M
