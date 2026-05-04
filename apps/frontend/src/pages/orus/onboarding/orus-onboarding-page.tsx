/**
 * Orus Onboarding Page
 * First-time setup wizard for new users
 */
import { useState } from "react";
import { useNavigate, Navigate } from "react-router-dom";
import { useOrusAuth, OrusUserProfile } from "@/features/orus-integration/orus-auth-context";
import {
  ApplicationShell,
  Button,
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  Input,
  Label,
  Icons,
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@wealthfolio/ui";
import { AnimatePresence, motion } from "motion/react";

// =====================================================
// CONSTANTS
// =====================================================

const CURRENCIES = [
  { value: "EUR", label: "Euro (€)" },
  { value: "USD", label: "Dollar US ($)" },
  { value: "CHF", label: "Franc Suisse (CHF)" },
  { value: "GBP", label: "Livre Sterling (£)" },
];

const LANGUAGES = [
  { value: "fr", label: "Français" },
  { value: "en", label: "English" },
];

const MAX_STEPS = 3;

// =====================================================
// STEP COMPONENTS
// =====================================================

interface StepProps {
  onNext: () => void;
  onBack?: () => void;
  profile: OrusUserProfile;
  updateLocalProfile: (data: Partial<OrusUserProfile>) => void;
}

function Step1Welcome({ onNext, profile, updateLocalProfile }: StepProps) {
  const [fullName, setFullName] = useState(profile.full_name || "");
  const [phone, setPhone] = useState(profile.phone || "");
  const [company, setCompany] = useState(profile.company || "");
  const [jobTitle, setJobTitle] = useState(profile.job_title || "");

  const handleNext = () => {
    updateLocalProfile({
      full_name: fullName,
      phone,
      company,
      job_title: jobTitle,
    });
    onNext();
  };

  return (
    <div className="w-full max-w-lg space-y-6">
      <div className="text-center space-y-2">
        <h2 className="text-2xl font-bold">Bienvenue sur Orus</h2>
        <p className="text-muted-foreground">
          Complétez votre profil pour commencer
        </p>
      </div>

      <Card>
        <CardContent className="pt-6 space-y-4">
          <div className="space-y-2">
            <Label htmlFor="fullName">Nom complet *</Label>
            <Input
              id="fullName"
              type="text"
              placeholder="Jean Dupont"
              value={fullName}
              onChange={(e) => setFullName(e.target.value)}
              className="h-12"
            />
          </div>

          <div className="space-y-2">
            <Label htmlFor="phone">Téléphone</Label>
            <Input
              id="phone"
              type="tel"
              placeholder="+33 6 12 34 56 78"
              value={phone}
              onChange={(e) => setPhone(e.target.value)}
              className="h-12"
            />
          </div>

          <div className="grid grid-cols-2 gap-4">
            <div className="space-y-2">
              <Label htmlFor="company">Société</Label>
              <Input
                id="company"
                type="text"
                placeholder="Ma Société"
                value={company}
                onChange={(e) => setCompany(e.target.value)}
                className="h-12"
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="jobTitle">Fonction</Label>
              <Input
                id="jobTitle"
                type="text"
                placeholder="Directeur"
                value={jobTitle}
                onChange={(e) => setJobTitle(e.target.value)}
                className="h-12"
              />
            </div>
          </div>
        </CardContent>
      </Card>

      <Button
        onClick={handleNext}
        className="w-full h-12"
        disabled={!fullName.trim()}
      >
        Continuer
        <Icons.ArrowRight className="ml-2 h-4 w-4" />
      </Button>
    </div>
  );
}

function Step2Preferences({ onNext, onBack, profile, updateLocalProfile }: StepProps) {
  const [currency, setCurrency] = useState(profile.preferred_currency || "EUR");
  const [language, setLanguage] = useState(profile.preferred_language || "fr");

  const handleNext = () => {
    updateLocalProfile({
      preferred_currency: currency,
      preferred_language: language,
    });
    onNext();
  };

  return (
    <div className="w-full max-w-lg space-y-6">
      <div className="text-center space-y-2">
        <h2 className="text-2xl font-bold">Vos préférences</h2>
        <p className="text-muted-foreground">
          Personnalisez votre expérience
        </p>
      </div>

      <Card>
        <CardContent className="pt-6 space-y-4">
          <div className="space-y-2">
            <Label>Devise préférée</Label>
            <Select value={currency} onValueChange={setCurrency}>
              <SelectTrigger className="h-12">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {CURRENCIES.map((c) => (
                  <SelectItem key={c.value} value={c.value}>
                    {c.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <p className="text-xs text-muted-foreground">
              Les montants seront affichés dans cette devise par défaut
            </p>
          </div>

          <div className="space-y-2">
            <Label>Langue</Label>
            <Select value={language} onValueChange={setLanguage}>
              <SelectTrigger className="h-12">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {LANGUAGES.map((l) => (
                  <SelectItem key={l.value} value={l.value}>
                    {l.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
        </CardContent>
      </Card>

      <div className="flex gap-3">
        <Button variant="ghost" onClick={onBack} className="flex-1 h-12">
          <Icons.ArrowLeft className="mr-2 h-4 w-4" />
          Retour
        </Button>
        <Button onClick={handleNext} className="flex-1 h-12">
          Continuer
          <Icons.ArrowRight className="ml-2 h-4 w-4" />
        </Button>
      </div>
    </div>
  );
}

function Step3Complete({
  onBack,
  profile,
  onFinish,
  isLoading,
}: StepProps & { onFinish: () => void; isLoading: boolean }) {
  return (
    <div className="w-full max-w-lg space-y-6">
      <div className="text-center space-y-4">
        <div className="flex justify-center">
          <div className="flex h-20 w-20 items-center justify-center rounded-full bg-green-100">
            <Icons.CheckCircle className="h-10 w-10 text-green-600" />
          </div>
        </div>
        <h2 className="text-2xl font-bold">Tout est prêt !</h2>
        <p className="text-muted-foreground">
          Votre compte est configuré. Vous pouvez maintenant accéder à toutes les fonctionnalités d'Orus.
        </p>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="text-lg">Récapitulatif</CardTitle>
        </CardHeader>
        <CardContent className="space-y-3">
          <div className="flex justify-between">
            <span className="text-muted-foreground">Nom</span>
            <span className="font-medium">{profile.full_name}</span>
          </div>
          <div className="flex justify-between">
            <span className="text-muted-foreground">Email</span>
            <span className="font-medium">{profile.email}</span>
          </div>
          {profile.company && (
            <div className="flex justify-between">
              <span className="text-muted-foreground">Société</span>
              <span className="font-medium">{profile.company}</span>
            </div>
          )}
          <div className="flex justify-between">
            <span className="text-muted-foreground">Devise</span>
            <span className="font-medium">
              {CURRENCIES.find((c) => c.value === profile.preferred_currency)?.label || profile.preferred_currency}
            </span>
          </div>
          <div className="flex justify-between">
            <span className="text-muted-foreground">Langue</span>
            <span className="font-medium">
              {LANGUAGES.find((l) => l.value === profile.preferred_language)?.label || profile.preferred_language}
            </span>
          </div>
        </CardContent>
      </Card>

      <div className="flex gap-3">
        <Button variant="ghost" onClick={onBack} className="h-12" disabled={isLoading}>
          <Icons.ArrowLeft className="mr-2 h-4 w-4" />
          Retour
        </Button>
        <Button onClick={onFinish} className="flex-1 h-12" disabled={isLoading}>
          {isLoading ? (
            <>
              <Icons.Spinner className="mr-2 h-4 w-4 animate-spin" />
              Finalisation...
            </>
          ) : (
            <>
              Commencer
              <Icons.ArrowRight className="ml-2 h-4 w-4" />
            </>
          )}
        </Button>
      </div>
    </div>
  );
}

// =====================================================
// MAIN COMPONENT
// =====================================================

export default function OrusOnboardingPage() {
  const navigate = useNavigate();
  const { profile, loading, isAuthenticated, updateProfile, completeOnboarding } = useOrusAuth();
  const [currentStep, setCurrentStep] = useState(1);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [localProfile, setLocalProfile] = useState<Partial<OrusUserProfile>>({});

  // Redirect if not authenticated
  if (!loading && !isAuthenticated) {
    return <Navigate to="/orus/auth/login" replace />;
  }

  // Redirect if onboarding already completed
  if (!loading && profile && profile.onboarding_completed) {
    return <Navigate to="/administration" replace />;
  }

  // Loading state
  if (loading || !profile) {
    return (
      <ApplicationShell className="fixed inset-0 flex items-center justify-center">
        <div className="flex flex-col items-center gap-4">
          <div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" />
          <p className="text-muted-foreground text-sm">Chargement...</p>
        </div>
      </ApplicationShell>
    );
  }

  const mergedProfile = { ...profile, ...localProfile };

  const updateLocalProfile = (data: Partial<OrusUserProfile>) => {
    setLocalProfile((prev) => ({ ...prev, ...data }));
  };

  const handleNext = () => {
    setCurrentStep((prev) => Math.min(prev + 1, MAX_STEPS));
  };

  const handleBack = () => {
    setCurrentStep((prev) => Math.max(prev - 1, 1));
  };

  const handleFinish = async () => {
    console.log("[Onboarding] handleFinish: Starting...");
    console.log("[Onboarding] handleFinish: localProfile =", localProfile);
    setIsSubmitting(true);
    try {
      // Update profile with all collected data
      console.log("[Onboarding] handleFinish: Calling updateProfile...");
      const { error: updateError } = await updateProfile(localProfile);
      if (updateError) {
        console.error("[Onboarding] handleFinish: Error updating profile:", updateError);
        // Continue anyway to complete onboarding
      } else {
        console.log("[Onboarding] handleFinish: Profile updated successfully");
      }

      // Mark onboarding as complete
      console.log("[Onboarding] handleFinish: Calling completeOnboarding...");
      const { error: completeError } = await completeOnboarding();
      if (completeError) {
        console.error("[Onboarding] handleFinish: Error completing onboarding:", completeError);
        return;
      }
      console.log("[Onboarding] handleFinish: Onboarding completed successfully");

      // Redirect to dashboard
      console.log("[Onboarding] handleFinish: Navigating to /administration...");
      navigate("/administration");
    } catch (error) {
      console.error("[Onboarding] handleFinish: Exception:", error);
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <ApplicationShell className="fixed inset-0 flex flex-col bg-gradient-to-b from-background to-muted/20">
      {/* Header */}
      <header className="flex-none px-4 pt-8 sm:px-6 sm:pt-12">
        <div className="flex flex-col items-center">
          {/* Logo */}
          <div className="flex h-16 w-16 items-center justify-center rounded-full bg-primary/10 mb-4">
            <Icons.Briefcase className="h-8 w-8 text-primary" />
          </div>

          {/* Progress indicators */}
          <div className="flex gap-2">
            {Array.from({ length: MAX_STEPS }).map((_, index) => (
              <div
                key={index}
                className={`h-1.5 rounded-full transition-all duration-300 ${
                  index === currentStep - 1
                    ? "bg-primary w-8"
                    : index < currentStep - 1
                      ? "bg-primary/50 w-1.5"
                      : "bg-muted w-1.5"
                }`}
              />
            ))}
          </div>
        </div>
      </header>

      {/* Main content */}
      <main className="flex flex-1 flex-col items-center justify-center overflow-y-auto px-4 py-8 sm:px-6">
        <AnimatePresence mode="wait" initial={false}>
          <motion.div
            key={currentStep}
            initial={{ opacity: 0, x: 20 }}
            animate={{ opacity: 1, x: 0 }}
            exit={{ opacity: 0, x: -20 }}
            transition={{ duration: 0.2 }}
            className="flex w-full justify-center"
          >
            {currentStep === 1 && (
              <Step1Welcome
                onNext={handleNext}
                profile={mergedProfile}
                updateLocalProfile={updateLocalProfile}
              />
            )}
            {currentStep === 2 && (
              <Step2Preferences
                onNext={handleNext}
                onBack={handleBack}
                profile={mergedProfile}
                updateLocalProfile={updateLocalProfile}
              />
            )}
            {currentStep === 3 && (
              <Step3Complete
                onNext={() => {}}
                onBack={handleBack}
                profile={mergedProfile}
                updateLocalProfile={updateLocalProfile}
                onFinish={handleFinish}
                isLoading={isSubmitting}
              />
            )}
          </motion.div>
        </AnimatePresence>
      </main>
    </ApplicationShell>
  );
}
