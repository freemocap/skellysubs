import {useAppDispatch, useAppSelector} from "../../../store/hooks";
import {useContext, useState} from "react";
import {RightPanelContext} from "../../../layout/BasePanelLayout";
import {
    selectAvailableTargetLanguages,
    selectSelectedTargetLanguages, toggleLanguage
} from "../../../store/slices/translation-config/translationConfigSlice";
import {Box, IconButton, Popover, Paper, Typography} from "@mui/material";
import Chip from "@mui/material/Chip";
import AddCircleIcon from "@mui/icons-material/AddCircle";

export const LanguageChipsPanel = () => {
    const dispatch = useAppDispatch()
    const { toggleRightPanel } = useContext(RightPanelContext)
    const availableTargetLanguages = useAppSelector(selectAvailableTargetLanguages)
    const selectedTargetLanguages = useAppSelector(selectSelectedTargetLanguages)

    // Add state for popover
    const [anchorEl, setAnchorEl] = useState<HTMLElement | null>(null);
    const [hoveredLanguage, setHoveredLanguage] = useState<string | null>(null);

    const handlePopoverOpen = (event: React.MouseEvent<HTMLElement>, configKey: string) => {
        setAnchorEl(event.currentTarget);
        setHoveredLanguage(configKey);
    };

    const handlePopoverClose = () => {
        setAnchorEl(null);
        setHoveredLanguage(null);
    };

    const open = Boolean(anchorEl);

    return (
        <Box sx={{ display: "flex", flexWrap: "wrap", gap: 1, mb: 2 }}>
            {selectedTargetLanguages.map(configKey => {
                const lang = availableTargetLanguages[configKey]
                return (
                    <Box key={configKey}>
                        <Chip
                            label={lang?.language_name || configKey}
                            onDelete={() => dispatch(toggleLanguage(configKey))}
                            sx={{ backgroundColor: "#005d5d" }}
                            onMouseEnter={(e) => handlePopoverOpen(e, configKey)}
                            onMouseLeave={handlePopoverClose}
                        />
                        <Popover
                            sx={{
                                pointerEvents: 'none',
                            }}
                            open={open && hoveredLanguage === configKey}
                            anchorEl={anchorEl}
                            anchorOrigin={{
                                vertical: 'bottom',
                                horizontal: 'left',
                            }}
                            transformOrigin={{
                                vertical: 'top',
                                horizontal: 'left',
                            }}
                            onClose={handlePopoverClose}
                            disableRestoreFocus
                        >
                            <Paper sx={{ p: 2, maxWidth: 300 }}>
                                <Typography variant="subtitle2" gutterBottom>
                                    {lang?.language_name} ({lang?.language_code})
                                </Typography>
                                {lang?.background?.family_tree && (
                                    <Typography variant="body2" gutterBottom>
                                        Family Tree: {lang.background.family_tree.join(" → ")}
                                    </Typography>
                                )}
                                {lang?.background?.alphabet && (
                                    <Typography variant="body2" gutterBottom>
                                        Alphabet: {lang.background.alphabet}
                                    </Typography>
                                )}
                                {lang?.background?.sample_text && (
                                    <Typography variant="body2" gutterBottom>
                                        Sample Text: {lang.background.sample_text}
                                    </Typography>
                                )}
                                {lang?.romanization_method !== "NONE" && lang?.background?.sample_romanized_text && (
                                    <Typography variant="body2" gutterBottom>
                                        Romanized: {lang.background.sample_romanized_text}
                                    </Typography>
                                )}
                            </Paper>
                        </Popover>
                    </Box>
                )
            })}
            <IconButton onClick={toggleRightPanel}>
                <AddCircleIcon />
            </IconButton>
        </Box>
    )
}
