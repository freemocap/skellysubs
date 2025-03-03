// PromptDrawer.tsx
import { Drawer, IconButton, Tooltip, Typography, TextareaAutosize, Box } from "@mui/material";
import { Info, Close } from "@mui/icons-material";

export const PromptEditDrawer = ({ open, onClose, prompt, setPrompt }: {
    open: boolean;
    onClose: () => void;
    prompt: string;
    setPrompt: (value: string) => void;
}) => (
    <Drawer
        anchor="right"
open={open}
onClose={onClose}
sx={{
    '& .MuiDrawer-paper': {
        width: 400,
            p: 3
    }
}}
>
<Box sx={{ display: 'flex', justifyContent: 'space-between', mb: 2 }}>
<Typography variant="h6">
    Transcription Prompt
<Tooltip title="Provide optional context to help the transcription (e.g., 'Names: John Smith, Company: Acme Corp')">
<Info sx={{ ml: 1, fontSize: 18 }} />
</Tooltip>
</Typography>
<IconButton onClick={onClose}>
    <Close />
    </IconButton>
    </Box>

    <TextareaAutosize
value={prompt}
onChange={(e) => setPrompt(e.target.value)}
placeholder="Enter optional context for the transcription..."
style={{
    width: '100%',
        minHeight: 200,
        padding: '8px',
        borderColor: '#ccc',
        borderRadius: '4px',
        fontFamily: 'inherit',
        fontSize: '14px'
}}
/>
</Drawer>
);