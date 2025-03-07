import {useAppDispatch, useAppSelector} from "../../../store/hooks";
import {useContext} from "react";
import {RightPanelContext} from "../../../layout/BasePanelLayout";
import {
    selectAvailableTargetLanguages,
    selectSelectedTargetLanguages, toggleLanguage
} from "../../../store/slices/translation-config/translationConfigSlice";
import {Box, IconButton} from "@mui/material";
import Chip from "@mui/material/Chip";
import AddCircleIcon from "@mui/icons-material/AddCircle";

export const LanguageChipsPanel = () => {
    const dispatch = useAppDispatch()
    const { toggleRightPanel } = useContext(RightPanelContext)
    const availableTargetLanguages = useAppSelector(selectAvailableTargetLanguages)
    const selectedTargetLanguages = useAppSelector(selectSelectedTargetLanguages)

    return (
        <Box sx={{ display: "flex", flexWrap: "wrap", gap: 1, mb: 2 }}>
            {selectedTargetLanguages.map(configKey => {
                const lang = availableTargetLanguages[configKey]
                return (
                    <Chip
                        key={configKey}
                        label={lang?.language_name || configKey}
                        onDelete={() => dispatch(toggleLanguage(configKey))}
                        sx={{ backgroundColor: "#005d5d" }}
                    />
                )
            })}
            <IconButton onClick={toggleRightPanel}>
                <AddCircleIcon />
            </IconButton>
        </Box>
    )
}
