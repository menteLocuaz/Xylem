local M = {}

M.rpc_callbacks = {}
M.message_queue = {}
M.is_connected = false

function M.connect()
    M.is_connected = true
    vim.defer_fn(function()
        M.process_queue()
    end, 10)
end

function M.disconnect()
    M.is_connected = false
    M.message_queue = {}
end

function M.send(method, params, callback)
    local message = {
        method = method,
        params = params,
    }

    if callback then
        local id = vim.fn.rand()
        M.rpc_callbacks[id] = callback
        message.id = id
    end

    local encoded = vim.json.encode(message) .. "\n"
    table.insert(M.message_queue, encoded)

    return true
end

function M.process_queue()
    if not M.is_connected then
        return
    end

    while #M.message_queue > 0 do
        local msg = table.remove(M.message_queue, 1)
        vim.fn.chansend(0, msg)
    end

    vim.defer_fn(function()
        M.process_queue()
    end, 50)
end

function M.on_response(id, result, error)
    if M.rpc_callbacks[id] then
        M.rpc_callbacks[id](result, error)
        M.rpc_callbacks[id] = nil
    end
end

function M.on_notification(method, params)
    local handler = M.rpc_callbacks["notification_" .. method]
    if handler then
        handler(params)
    end
end

function M.wrap_async(fn)
    return function(...)
        local co = coroutine.create(fn)
        local args = {...}
        local success, err = coroutine.resume(co, unpack(args))
        if not success then
            vim.notify("[xylem-rpc] Error: " .. tostring(err), vim.log.levels.ERROR)
        end
    end
end

return M