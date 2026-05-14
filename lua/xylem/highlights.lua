local M = {}

M.ns_id = nil
M.highlight_cache = {}
M.enabled = true

function M.setup()
    M.ns_id = vim.api.nvim_create_namespace("xylem_highlights")
end

function M.apply_highlights(buf_id, highlights)
    if not M.enabled then
        return
    end

    if not vim.api.nvim_buf_is_loaded(buf_id) then
        return
    end

    vim.highlight.clear(M.get_ns(), buf_id)

    for _, hl in ipairs(highlights) do
        local start_pos = M.byte_to_pos(buf_id, hl.start_byte)
        local end_pos = M.byte_to_pos(buf_id, hl.end_byte)

        local extmark_id = vim.api.nvim_buf_set_extmark(
            buf_id,
            M.get_ns(),
            start_pos[1],
            start_pos[2],
            {
                end_row = end_pos[1],
                end_col = end_pos[2],
                hl_group = hl.hl_group,
            }
        )
    end

    M.highlight_cache[buf_id] = highlights
end

function M.clear_highlights(buf_id)
    if M.highlight_cache[buf_id] then
        vim.highlight.clear(M.get_ns(), buf_id)
        M.highlight_cache[buf_id] = nil
    end
end

function M.get_ns()
    if not M.ns_id then
        M.setup()
    end
    return M.ns_id
end

function M.byte_to_pos(buf_id, byte)
    local lines = vim.api.nvim_buf_get_lines(buf_id, 0, -1, false)
    local current_byte = 0

    for i, line in ipairs(lines) do
        local line_bytes = #line + 1
        if current_byte + line_bytes > byte then
            return { i - 1, byte - current_byte }
        end
        current_byte = current_byte + line_bytes
    end

    return { 0, 0 }
end

function M.set_enabled(enabled)
    M.enabled = enabled
end

function M.is_enabled()
    return M.enabled
end

vim.api.nvim_create_user_command(
    "XylemHighlightsEnable",
    function()
        M.set_enabled(true)
        vim.notify("[xylem] Highlights enabled", vim.log.levels.INFO)
    end,
    {}
)

vim.api.nvim_create_user_command(
    "XylemHighlightsDisable",
    function()
        M.set_enabled(false)
        vim.notify("[xylem] Highlights disabled", vim.log.levels.INFO)
    end,
    {}
)

return M