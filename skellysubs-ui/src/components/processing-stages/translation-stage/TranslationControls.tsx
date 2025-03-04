// src/components/processing-panel/translation/TranslationControls.tsx
import { Box } from "@mui/material";
import type { LanguageConfig } from "../../../schemas/languageConfigSchemas";
import { LanguageConfigList } from "./LanguageConfigList";

export const TranslationControls = ({
                                        languageOptions,
                                        setLanguageOptions,
                                        targetLanguages,
                                        setTargetLanguages,
                                    }: {
    languageOptions: LanguageConfig[];
    setLanguageOptions: (configs: LanguageConfig[]) => void;
    targetLanguages: string;
    setTargetLanguages: (v: string) => void;
}) => {
    return (
        <Box sx={{ width: "100%", mb: 2 }}>
            <LanguageConfigList
                languageOptions={languageOptions}
                setLanguageOptions={setLanguageOptions}
                targetLanguages={targetLanguages}
                setTargetLanguages={setTargetLanguages}
            />
        </Box>
    );
};