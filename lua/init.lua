local args = {...};
local serv = args[1];

-- install dependencies
if not fs.exists("/deps/libDeflate") then
    fs.makeDir("/deps");
    shell.run("wget http://" .. serv .. "/deps/libDeflate.lua /deps/libDeflate.lua");
end

local max_file_size = 61072; 

function iter_all_fs(on_file,from) 

    for _,v in next, fs.list(from) do

        local combined = fs.combine(from,v)

        if fs.isDir(combined) then

            iter_all_fs(on_file,combined)

        else

            on_file(combined)

        end

    end

end

print("Connecting to " .. serv);
local ws = http.websocket("ws://" .. serv .. "/ws");

if ws == nil then
    print("Failed to connect to " .. serv);
    return;
end

local libDeflate = require("/deps/libDeflate");

ws.send(textutils.serializeJSON({p = {["C2sComp"] = os.getComputerID()}}),true);
while true do 

    local msg = ws.receive();



    local decompressed = libDeflate:DecompressGzip(msg);
    local parsed = textutils.unserializeJSON(decompressed);
    if parsed.p == "S2cHb" then
        print("Heartbeat");
        ws.send(textutils.serializeJSON({p = "C2sHb"}),true);
    elseif parsed.p["S2cSyncF"] then
        local file = parsed.p["S2cSyncF"];
        if file.read_only then 
            if file.data == "dir" then 
                -- create dir
                fs.makeDir(file.name);
            else
                -- delete
                fs.delete(file.name);
            end
        
        end
        if fs.exists(file.name) then 
        
            local f2 = fs.open(file.name,"r");
            local data = f2.readAll();
            f2.close();
            if data == file.data then 
                print("File " .. file.name .. " is up to date");
            else
                print("File " .. file.name .. " is out of date");
                local f2 = fs.open(file.name,"w");
                f2.write(file.data);
                f2.close();
            end
        else
            print("File " .. file.name .. " does not exist, creating");
            local dir = fs.getDir(file.name);
            if not fs.exists(dir) then
                fs.makeDir(dir);
            end
            local f2 = fs.open(file.name,"w");
            f2.write(file.data);
            f2.close();
        end        
    elseif parsed.p == "S2cSync" then
        print("Message: " .. decompressed);
        iter_all_fs(function(f) 
        
            local ro = fs.isReadOnly(f);
            local f2 = fs.open(f,"r");
            local data = f2.readAll();
            
            f2.close();
            if #data > max_file_size then
                print("File " .. f .. " is too large, sending partials");
                for i = 1, #data, max_file_size do
                    local partial = data:sub(i,i+max_file_size-1);
                    print("Sending partial " .. i .. " of " .. #data .. " bytes");
                    local finished = i+max_file_size-1 >= #data;
                    print("data size = " .. #partial)
                    print("finished = " .. tostring(finished))
                    ws.send(textutils.serializeJSON({p = {["C2sSync"] = {Partial = {{name=f,data=partial,read_only=ro},finished}}}}),true); 
                end
            else
                print("Sending file " .. f .. " of " .. #data .. " bytes");
                ws.send(textutils.serializeJSON({p = {["C2sSync"] = {Full={name=f,data=data,read_only=ro}}}}),true);
            end
            -- pcall(function() 
            --     ws.send(textutils.serializeJSON({p = {["C2sSync"] = {name=f,data=data,read_only=ro}}}));
            -- end)
        
        end,"/")
        os.sleep(1);
        ws.send(textutils.serializeJSON({p = "C2sReady"}),true);
    end

    os.sleep(0.1);

end