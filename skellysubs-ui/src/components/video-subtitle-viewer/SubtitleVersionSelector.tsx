// skellysubs-ui/src/components/video-subtitle-viewer/SubtitleVersionSelector.tsx
import type React from "react";
import { useState } from "react";
import {
    Box,
    Button,
    Menu,
    MenuItem,
    Typography,
    Stack,
    ListItemText,
    CardContent,
} from "@mui/material";
import { Save as SaveIcon, PlayArrow as PlayArrowIcon } from "@mui/icons-material";
import type { SubtitleCue } from "./video-subtitle-viewer-types";
import extendedPaperbaseTheme from "../../layout/paperbase_theme/paperbase-theme";

interface SubtitleVersionSelectorProps {
    subtitles: SubtitleCue[];
    selectedId?: string | null;
    onSelect: (id: string) => void;
}

export const SubtitleVersionSelector = ({
    subtitles,
    selectedId,
    onSelect
}: SubtitleVersionSelectorProps) => {
    const [anchorEl, setAnchorEl] = useState<null | HTMLElement>(null);

    const handleOpenMenu = (event: React.MouseEvent<HTMLElement>) => {
        setAnchorEl(event.currentTarget);
    };

    const handleCloseMenu = () => {
        setAnchorEl(null);
    };

    const handleSelect = (id: string) => {
        onSelect(id);
        handleCloseMenu();
    };

    const handleSave = () => {
        const selectedSub = subtitles.find(sub => sub.id === selectedId);
        if (!selectedSub) return;

        const blob = new Blob([selectedSub.content], { type: "text/vtt" });
        const url = URL.createObjectURL(blob);
        const a = document.createElement("a");
        a.href = url;
        a.download = `${selectedSub.name.replace(/\s+/g, "_")}_subtitles.vtt`;
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
        URL.revokeObjectURL(url);
    };

    const selectedSubtitle = subtitles.find(sub => sub.id === selectedId);

    return (
        <CardContent
            sx={{
                background: "rgba(0, 0, 0, 0.75)",
                borderRadius: 1,
                borderColor: "rgba(255, 255, 255, 0.5)",
                borderWidth: 1,
                borderStyle: "solid",
            }}
        >
            <Stack direction="row" spacing={2} alignItems="center" justifyContent="space-between">
                <Box>
                    <Typography variant="subtitle1" gutterBottom color="primary.contrastText">
                        Current Version:
                    </Typography>
                    <Typography variant="body2" color="primary.contrastText">
                        {selectedSubtitle?.name || "None selected"}
                    </Typography>
                </Box>
                <Stack direction="row" spacing={1}>
                    <Button
                        variant="contained"
                        onClick={handleOpenMenu}
                        startIcon={<PlayArrowIcon />}
                        sx={{
                            backgroundColor: extendedPaperbaseTheme.palette.primary.light,
                            '&:hover': {
                                backgroundColor: 'primary.dark',
                            }
                        }}
                    >
                        Select Version
                    </Button>
                    <Button
                        variant="contained"
                        startIcon={<SaveIcon />}
                        onClick={handleSave}
                        disabled={!selectedId}
                        sx={{
                            backgroundColor: extendedPaperbaseTheme.palette.primary.light,
                            '&:hover': {
                                backgroundColor: 'primary.dark',
                            }
                        }}
                    >
                        Save As
                    </Button>
                </Stack>
            </Stack>

            <Menu
                anchorEl={anchorEl}
                open={Boolean(anchorEl)}
                onClose={handleCloseMenu}
                PaperProps={{
                    sx: { maxWidth: 300, maxHeight: 400 }
                }}
            >
                <Typography variant="caption" sx={{ px: 2, py: 0.5, display: 'block', color: 'text.secondary' }}>
                    Available Versions
                </Typography>
                {subtitles.map((sub) => (
                    <MenuItem
                        key={sub.id}
                        onClick={() => handleSelect(sub.id)}
                        selected={sub.id === selectedId}
                        dense
                    >
                        <ListItemText
                            primary={sub.name}
                            secondary={`Format: ${sub.format.toUpperCase()}`}
                        />
                    </MenuItem>
                ))}
            </Menu>
        </CardContent>
    );
};
